use crate::{config, frame, sources};
use anyhow::Result;
use chrono_tz::Tz;
use rand::rng;
use rand::seq::SliceRandom;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::info;

type SharedImageSource = Arc<Box<dyn sources::ImageSource>>;
type SourcesMap = HashMap<String, SharedImageSource>;
type SharedSourcesMap = Arc<RwLock<SourcesMap>>;

pub struct FrameScheduler {
    sched: JobScheduler,
    cfg: config::SharedConfig,
    pub(crate) sources: SharedSourcesMap,
}

impl FrameScheduler {
    pub async fn new(cfg: config::SharedConfig) -> Result<Self> {
        let sched = JobScheduler::new().await?;
        let sources_map = Self::build_sources_map(&cfg).await?;
        Ok(Self {
            sched,
            cfg,
            sources: Arc::new(RwLock::new(sources_map)),
        })
    }

    /// Parse timezone from TZ environment variable, fallback to UTC
    fn get_timezone() -> Result<Tz> {
        if let Ok(tz_str) = std::env::var("TZ") {
            match tz_str.parse::<Tz>() {
                Ok(tz) => {
                    tracing::info!(timezone = %tz, "using timezone from TZ environment variable");
                    Ok(tz)
                }
                Err(e) => {
                    tracing::warn!(tz = %tz_str, error = %e, "invalid timezone in TZ environment variable, falling back to UTC");
                    Ok(chrono_tz::UTC)
                }
            }
        } else {
            tracing::info!("no TZ environment variable set, using UTC");
            Ok(chrono_tz::UTC)
        }
    }

    /// Build the sources map from the current configuration
    async fn build_sources_map(cfg: &config::SharedConfig) -> Result<SourcesMap> {
        let snapshot = config::ConfigManager::to_struct(cfg).await?;
        let mut map: SourcesMap = HashMap::new();
        for (id, src_cfg) in snapshot.sources.iter() {
            match sources::build_source(src_cfg) {
                Ok(built) => {
                    map.insert(id.clone(), Arc::new(built));
                }
                Err(e) => {
                    tracing::warn!(source = %id, error = %e, "failed to build source");
                }
            }
        }
        Ok(map)
    }

    /// Reload all sources from the current configuration
    pub async fn reload_sources(&self) -> Result<()> {
        tracing::info!("reloading sources from configuration");
        let new_sources_map = Self::build_sources_map(&self.cfg).await?;

        // Replace the sources map atomically
        {
            let mut sources_guard = self.sources.write().await;
            *sources_guard = new_sources_map;
        }

        tracing::info!("sources reloaded successfully");
        Ok(())
    }

    pub async fn populate(&self) -> Result<()> {
        let cfg_snapshot = config::ConfigManager::to_struct(&self.cfg).await?;
        let timezone = Self::get_timezone()?;

        for (frame_id, frame) in cfg_snapshot.photoframes.iter() {
            if let Some(cron) = &frame.update_cron {
                let frame_id_clone = frame_id.clone();
                let shared = Arc::clone(&self.cfg);
                let sources_map = Arc::clone(&self.sources);
                let cron_expr = cron.to_string();
                let job = Job::new_async_tz(cron_expr.as_str(), timezone, move |_uuid, _l| {
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
        sources_map: &SharedSourcesMap,
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
        {
            let sources_guard = sources_map.read().await;
            for sid in &f.source_ids {
                if let Some(src) = sources_guard.get(sid) {
                    let st = src.stats();
                    tracing::debug!(frame=%frame_id, source=%sid, total=st.total, landscape=st.landscape, portrait=st.portrait, "source stats");
                } else {
                    tracing::warn!(frame=%frame_id, source=%sid, "configured source id not found in scheduler map");
                }
            }
        }

        let mut selected: Option<sources::ImageMeta> = None;

        // Shuffle configured sources before probing to select a source at random
        let mut sids: Vec<String> = f.source_ids.to_vec();
        {
            let mut rng = rng();
            sids.shuffle(&mut rng);
        }

        // Process each source ID sequentially
        for sid in &sids {
            // Get a clone of the Arc for this specific source
            let source_arc = {
                let sources_guard = sources_map.read().await;
                sources_guard.get(sid).cloned()
            };

            if let Some(src) = source_arc
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

    /// Public method to manually trigger a schedule update for a frame id.
    /// This behaves exactly like the scheduled cron jobs - always fetches next image from sources.
    pub async fn manual_schedule_trigger(&self, frame_id: &str) -> Result<()> {
        Self::run_frame_update(&self.cfg, &self.sources, frame_id, true).await
    }

    pub async fn refresh_source(&self, source_id: &str) -> Result<()> {
        let source_arc = {
            let sources_guard = self.sources.read().await;
            sources_guard.get(source_id).cloned()
        };

        if let Some(src) = source_arc
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

        let mut selected: Option<sources::ImageMeta> = None;
        // Shuffle configured sources before probing to select a source at random
        let mut sids: Vec<String> = f.source_ids.to_vec();
        {
            let mut rng = rng();
            sids.shuffle(&mut rng);
        }

        for sid in &sids {
            // Get a clone of the Arc for this specific source
            let source_arc = {
                let sources_guard = self.sources.read().await;
                sources_guard.get(sid).cloned()
            };

            if let Some(src) = source_arc
                && let Ok(Some(meta)) = src.next(desired).await
            {
                selected = Some(meta);
                break;
            }
        }

        // Log stats to help diagnose empty selections.
        {
            let sources_guard = self.sources.read().await;
            for sid in &f.source_ids {
                if let Some(src) = sources_guard.get(sid) {
                    let st = src.stats();
                    tracing::debug!(frame=%frame_id, source=%sid, total=st.total, landscape=st.landscape, portrait=st.portrait, "source stats (prime)");
                }
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
            let date_taken = crate::frame::get_cached_date_taken(frame_id).await;
            let prepared = frame::prepare_from_base_with_date(f, &base, date_taken);
            let _ = frame::save_prepared(frame_id, &prepared);
            frame::push_to_device(frame_id, f, &prepared).await?;
        }
        Ok(())
    }
}
