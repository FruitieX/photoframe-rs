use crate::{config, frame, sources};
use anyhow::Result;
use rand::rng;
use rand::seq::SliceRandom;
use std::collections::HashMap;
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::info;

pub struct FrameScheduler {
    sched: JobScheduler,
    cfg: config::SharedConfig,
    pub(crate) sources: Arc<HashMap<String, Box<dyn sources::ImageSource>>>,
}

impl FrameScheduler {
    pub async fn new(cfg: config::SharedConfig) -> Result<Self> {
        let sched = JobScheduler::new().await?;
        // build sources from config snapshot once (later we can watch for changes)
        let snapshot = config::ConfigManager::to_struct(&cfg).await?;
        let mut map: HashMap<String, Box<dyn sources::ImageSource>> = HashMap::new();
        for (id, src_cfg) in snapshot.sources.iter() {
            match sources::build_source(src_cfg) {
                Ok(built) => {
                    map.insert(id.clone(), built);
                }
                Err(e) => {
                    tracing::warn!(source = %id, error = %e, "failed to build source");
                }
            }
        }
        Ok(Self {
            sched,
            cfg,
            sources: Arc::new(map),
        })
    }

    pub async fn populate(&self) -> Result<()> {
        let cfg_snapshot = config::ConfigManager::to_struct(&self.cfg).await?;
        for (frame_id, frame) in cfg_snapshot.photoframes.iter() {
            if let Some(cron) = &frame.update_cron {
                let frame_id_clone = frame_id.clone();
                let shared = Arc::clone(&self.cfg);
                let sources_map = Arc::clone(&self.sources);
                let cron_expr = cron.to_string();
                let job = Job::new_async(cron_expr.as_str(), move |_uuid, _l| {
                    let frame_id = frame_id_clone.clone();
                    let shared = Arc::clone(&shared);
                    let sources_map = Arc::clone(&sources_map);
                    Box::pin(async move {
                        if let Err(e) = FrameScheduler::run_frame_update(
                            &shared,
                            &sources_map,
                            &frame_id,
                            false,
                        )
                        .await
                        {
                            tracing::warn!(frame = %frame_id, error = %e, "frame update job failed");
                        }
                    })
                })?;
                self.sched.add(job).await?;
            }
        }
        Ok(())
    }

    pub async fn start(&self) -> Result<()> {
        self.sched.start().await?;
        Ok(())
    }

    /// Execute one update cycle for a specific frame id.
    async fn run_frame_update(
        cfg: &config::SharedConfig,
        sources_map: &HashMap<String, Box<dyn sources::ImageSource>>,
        frame_id: &str,
        ignore_pause: bool,
    ) -> Result<()> {
        let cfg_now = config::ConfigManager::to_struct(cfg).await?;
        let Some(f) = cfg_now.photoframes.get(frame_id) else {
            info!(frame = %frame_id, "frame not found at trigger time");
            return Ok(());
        };
        if f.paused && !ignore_pause {
            tracing::info!(frame=%frame_id, "frame paused; skipping scheduled update");
            return Ok(());
        }
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "<err>".into());
        tracing::debug!(frame = %frame_id, cwd = %cwd, sources = ?f.source_ids, orientation = ?f.orientation, "starting frame update cycle");
        let desired = f.orientation.unwrap_or_default();
        // Log stats for each configured source to diagnose empty selections.
        for sid in &f.source_ids {
            if let Some(src) = sources_map.get(sid) {
                let st = src.stats();
                tracing::debug!(frame=%frame_id, source=%sid, total=st.total, landscape=st.landscape, portrait=st.portrait, "source stats");
            } else {
                tracing::warn!(frame=%frame_id, source=%sid, "configured source id not found in scheduler map");
            }
        }
        let mut selected: Option<sources::ImageMeta> = None;

        // Shuffle configured sources before probing to select a source at random
        let mut sids: Vec<String> = f.source_ids.to_vec();
        {
            let mut rng = rng();
            sids.shuffle(&mut rng);
        }
        for sid in &sids {
            if let Some(src) = sources_map.get(sid)
                && let Ok(Some(meta)) = src.next(desired).await
            {
                selected = Some(meta);
                break;
            }
        }
        if f.source_ids.is_empty() {
            tracing::warn!(frame = %frame_id, "no sources configured for frame");
        }
        if let Some(meta) = &selected {
            let limits = cfg_now.image_limits.as_ref();
            if let Err(e) = crate::frame::process_and_push(frame_id, f, meta, limits).await {
                tracing::warn!(frame = %frame_id, error = %e, "failed to push image to frame");
            }
        }
        if selected.is_none() {
            tracing::warn!(frame = %frame_id, desired = ?desired, "no matching image found for update");
        }
        info!(frame = %frame_id, desired = ?desired, selected = ?selected, "frame cron triggered");
        Ok(())
    }

    /// Public method to manually trigger an update for a frame id.
    pub async fn trigger_frame(&self, frame_id: &str) -> Result<()> {
        // Prefer pushing the currently cached base image (from upload or prime-next)
        let cfg_now = config::ConfigManager::to_struct(&self.cfg).await?;
        if let Some(f) = cfg_now.photoframes.get(frame_id)
            && let Some(base) = frame::get_base_image(frame_id).await?
        {
            let prepared = frame::prepare_from_base(f, &base);
            if let Err(e) = frame::save_prepared(frame_id, &prepared) {
                tracing::warn!(frame=%frame_id, error=%e, "failed saving prepared image (trigger)");
            }
            frame::push_to_device(frame_id, f, &prepared).await?;
            tracing::info!(frame=%frame_id, "pushed currently cached image");
            return Ok(());
        }
        // Fallback: select next from sources and push
        Self::run_frame_update(&self.cfg, &self.sources, frame_id, true).await
    }

    pub async fn refresh_source(&self, source_id: &str) -> Result<()> {
        if let Some(src) = self.sources.get(source_id)
            && let Some(im) =
                (src.as_ref() as &dyn std::any::Any).downcast_ref::<sources::ImmichImageSource>()
        {
            im.refresh().await.ok();
        }
        Ok(())
    }

    /// Select the next image for a frame and cache it as the new base image without pushing to the device.
    pub async fn prime_next_image(&self, frame_id: &str) -> Result<()> {
        let cfg_now = config::ConfigManager::to_struct(&self.cfg).await?;
        let Some(f) = cfg_now.photoframes.get(frame_id) else {
            tracing::info!(frame=%frame_id, "frame not found at prime time");
            return Ok(());
        };
        let desired = f.orientation.unwrap_or_default();
        // Log stats to help diagnose empty selections.
        for sid in &f.source_ids {
            if let Some(src) = self.sources.get(sid) {
                let st = src.stats();
                tracing::debug!(frame=%frame_id, source=%sid, total=st.total, landscape=st.landscape, portrait=st.portrait, "source stats (prime)");
            }
        }
        let mut selected: Option<sources::ImageMeta> = None;
        // Shuffle configured sources before probing to select a source at random
        let mut sids: Vec<String> = f.source_ids.to_vec();
        {
            let mut rng = rng();
            sids.shuffle(&mut rng);
        }
        for sid in &sids {
            if let Some(src) = self.sources.get(sid)
                && let Ok(Some(meta)) = src.next(desired).await
            {
                selected = Some(meta);
                break;
            }
        }
        if selected.is_none() {
            tracing::warn!(frame=%frame_id, desired=?desired, "no matching image found to prime");
            return Ok(());
        }
        let meta = selected.unwrap();
        // Load and store base, and also write intermediate snapshot for UI toggle.
        let limits = cfg_now.image_limits.as_ref();
        let base = frame::load_and_store_base(frame_id, &meta, f, limits).await?;
        if let Err(e) = frame::save_intermediate_from_base(frame_id, f, &base).await {
            tracing::warn!(frame=%frame_id, error=%e, "failed saving intermediate image (prime)");
        }
        // Provide richer context about the chosen image for observability.
        match &meta.data {
            crate::sources::SourceData::Path(p) => {
                tracing::info!(frame=%frame_id, path=%p.display(), asset_id=?meta.id, orientation=?meta.orientation, "primed next image");
            }
            crate::sources::SourceData::Bytes(_) => {
                tracing::info!(frame=%frame_id, asset_id=?meta.id, orientation=?meta.orientation, "primed next image");
            }
        }
        Ok(())
    }

    /// Push the currently cached base image to the device, if any; otherwise no-op.
    pub async fn push_cached_base(&self, frame_id: &str) -> Result<()> {
        let cfg_now = config::ConfigManager::to_struct(&self.cfg).await?;
        let Some(f) = cfg_now.photoframes.get(frame_id) else {
            return Ok(());
        };
        if let Some(base) = crate::frame::get_base_image(frame_id).await? {
            let prepared = frame::prepare_from_base(f, &base);
            let _ = frame::save_prepared(frame_id, &prepared);
            frame::push_to_device(frame_id, f, &prepared).await?;
        }
        Ok(())
    }
}
