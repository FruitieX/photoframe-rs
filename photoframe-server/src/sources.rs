use crate::config::{FilesystemSource, ImmichSource, OrderKind, Orientation, Source};
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use glob::glob;
use image::ImageDecoder;
use rand::seq::{IndexedRandom, SliceRandom};
use rand::{Rng, rng};
use std::any::Any;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub enum SourceData {
    Path(PathBuf),
    Bytes(Vec<u8>),
}

#[derive(Clone)]
pub struct ImageMeta {
    pub data: SourceData,
    pub orientation: Orientation,
    /// Date taken extracted from EXIF or other metadata sources
    pub date_taken: Option<chrono::DateTime<chrono::Utc>>,
    /// Full EXIF blob from original image (for preserving in base PNG)
    pub exif_blob: Option<Vec<u8>>,
    #[allow(dead_code)]
    pub id: Option<String>,
}

impl Debug for ImageMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageMeta")
            .field("orientation", &self.orientation)
            .field("id", &self.id)
            .finish()
    }
}

impl Orientation {
    pub fn from_dims(w: u32, h: u32) -> Self {
        if w > h {
            Orientation::Landscape
        } else {
            Orientation::Portrait
        }
    }
}

/// Basic statistics for a source (debug aid).
#[derive(Debug, Clone, Copy, Default)]
pub struct SourceStats {
    pub total: usize,
    pub landscape: usize,
    pub portrait: usize,
}

impl SourceStats {
    pub fn from_entries(entries: &[ImageMeta]) -> Self {
        let total = entries.len();
        let mut landscape = 0;
        let mut portrait = 0;
        for e in entries {
            match e.orientation {
                Orientation::Landscape => landscape += 1,
                Orientation::Portrait => portrait += 1,
            }
        }

        SourceStats {
            total,
            landscape,
            portrait,
        }
    }
}

/// Unified trait for any image source.
#[async_trait]
pub trait ImageSource: Send + Sync + Any {
    /// Return the next image whose orientation matches or `Ok(None)` if none
    /// can be produced right now. Implementors should make a bounded effort per call.
    async fn next(&self, desired: Orientation) -> Result<Option<ImageMeta>>;
    /// Lightweight stat snapshot (override where meaningful).
    fn stats(&self) -> SourceStats {
        SourceStats::default()
    }
}

/// Filesystem implementation (simple, scans once then picks according to order).
pub struct FilesystemImageSource {
    pub entries: Vec<ImageMeta>,
    pub order: OrderKind,
    pub cursor: AtomicUsize,
}

impl FilesystemImageSource {
    /// Build a filesystem image source by eagerly expanding the configured glob
    /// and caching dimension-derived orientation metadata in memory.
    pub fn new(cfg: &FilesystemSource) -> Result<Self> {
        let glob_pat = cfg
            .glob
            .clone()
            .ok_or_else(|| anyhow::anyhow!("filesystem source missing glob"))?;
        let mut entries = Vec::new();
        tracing::info!(pattern = %glob_pat, "evaluating glob for filesystem source");
        match glob(&glob_pat).with_context(|| format!("evaluating glob {glob_pat}")) {
            Ok(paths) => {
                for path in paths.flatten() {
                    if let Ok(dim) = image::image_dimensions(&path) {
                        let orient = Orientation::from_dims(dim.0, dim.1);
                        entries.push(ImageMeta {
                            data: SourceData::Path(path.clone()),
                            orientation: orient,
                            date_taken: None, // Filesystem source doesn't extract EXIF during listing
                            exif_blob: None,  // Will be extracted when loading the file
                            id: Some(path.to_string_lossy().to_string()),
                        });
                    }
                }
            }
            Err(e) => {
                tracing::warn!(pattern = %glob_pat, error = %e, "glob evaluation failed");
            }
        }

        if entries.is_empty() {
            tracing::warn!(pattern = %glob_pat, "no images matched filesystem source glob");
        }
        let (mut l, mut p) = (0, 0);
        for e in &entries {
            match e.orientation {
                Orientation::Landscape => l += 1,
                Orientation::Portrait => p += 1,
            }
        }
        tracing::info!(pattern = %glob_pat, total = entries.len(), landscape = l, portrait = p, "filesystem source loaded");
        let order = cfg.order.unwrap_or_default();
        if matches!(order, OrderKind::Random) {
            let mut rng = rng();
            entries.shuffle(&mut rng);
        }
        Ok(Self {
            entries,
            order,
            cursor: AtomicUsize::new(0),
        })
    }
}

#[async_trait]
impl ImageSource for FilesystemImageSource {
    async fn next(&self, desired: Orientation) -> Result<Option<ImageMeta>> {
        if self.entries.is_empty() {
            return Ok(None);
        }
        match self.order {
            OrderKind::Sequential => {
                let total = self.entries.len();
                let start = self.cursor.fetch_add(1, AtomicOrdering::Relaxed);
                for offset in 0..total {
                    let idx = (start + offset) % total;
                    let item = &self.entries[idx];
                    if item.orientation == desired {
                        // advance cursor to after this idx (already incremented once above, so add remaining offset)
                        if offset > 0 {
                            self.cursor.fetch_add(offset, AtomicOrdering::Relaxed);
                        }
                        return Ok(Some(item.clone()));
                    }
                }
                Ok(None)
            }
            OrderKind::Random => {
                // random sample until match or attempts exhausted
                let mut rng = rng();
                for _ in 0..std::cmp::min(32, self.entries.len()) {
                    if let Some(item) = self
                        .entries
                        .choose(&mut rng)
                        .filter(|i| i.orientation == desired)
                    {
                        return Ok(Some(item.clone()));
                    }
                }
                Ok(None)
            }
        }
    }

    fn stats(&self) -> SourceStats {
        SourceStats::from_entries(&self.entries)
    }
}

pub struct ImmichImageSource {
    pub cfg: ImmichSource,
    pub entries: parking_lot::RwLock<Vec<(String, Orientation)>>, // asset_id + orientation metadata
    pub last_list: AtomicU64, // unix seconds of last listing, 0 = never
    pub cursor: AtomicUsize,  // for sequential order
}

impl ImmichImageSource {
    pub fn new(cfg: &ImmichSource) -> Result<Self> {
        Ok(Self {
            cfg: cfg.clone(),
            entries: parking_lot::RwLock::new(Vec::new()),
            last_list: AtomicU64::new(0),
            cursor: AtomicUsize::new(0),
        })
    }

    pub async fn refresh(&self) -> Result<()> {
        // force next call to list to actually list now
        self.last_list
            .store(0, std::sync::atomic::Ordering::Relaxed);
        self.list_if_needed().await
    }

    async fn list_if_needed(&self) -> Result<()> {
        if self.cfg.base_url.is_none() {
            return Ok(());
        }
        let mut should = false;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        const IMMICH_REFRESH_INTERVAL_SECS: u64 = 86_400; // 24h
        let last = self.last_list.load(std::sync::atomic::Ordering::Relaxed);
        if last == 0 || now.saturating_sub(last) > IMMICH_REFRESH_INTERVAL_SECS {
            should = true;
        }
        if !should {
            return Ok(());
        }

        let client = reqwest::Client::new();
        let base = self.cfg.base_url.clone().unwrap();
        let url = format!("{}/api/search/metadata", base.trim_end_matches('/'));

        // Handle multiple filters by performing multiple searches and deduplicating results
        let empty_filters = vec![];
        let filters_list = self.cfg.filters.as_ref().unwrap_or(&empty_filters);

        let mut all_entries = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        // If no filters are configured, perform a single search with just type=IMAGE
        let searches = if filters_list.is_empty() {
            vec![serde_json::Value::Object(serde_json::Map::new())]
        } else {
            filters_list.clone()
        };

        for (filter_idx, filter) in searches.iter().enumerate() {
            tracing::debug!(?filter, filter_idx, "Starting Immich search for filter");

            // Build filters body: merge user-provided filters (object) + enforced type=IMAGE.
            // We'll loop over pages and update the page field each iteration.
            let mut base = serde_json::Map::new();
            if let Some(obj) = filter.as_object() {
                for (k, v) in obj.iter() {
                    base.insert(k.clone(), v.clone());
                }
            }
            base.insert(
                "type".to_string(),
                serde_json::Value::String("IMAGE".to_string()),
            );

            // Defaults
            // Use a generic page token to support cursor-based pagination (nextPage: String|null).
            // If the filter specified an explicit page, start from there; otherwise omit 'page' on first call.
            let mut page_token: Option<serde_json::Value> = base.get("page").cloned();
            let size: u32 = base.get("size").and_then(|v| v.as_u64()).unwrap_or(1000) as u32;
            if !base.contains_key("withExif") {
                base.insert("withExif".to_string(), serde_json::Value::Bool(true));
            }

            let max_pages = self.cfg.max_pages.unwrap_or(1).max(1);
            let mut fetched_pages: u32 = 0;
            let mut total_assets_for_filter = 0;

            loop {
                if fetched_pages >= max_pages {
                    tracing::trace!(
                        filter_idx,
                        fetched_pages,
                        max_pages,
                        "Reached maximum pages limit for filter"
                    );
                    break;
                }
                let mut body_map = base.clone();
                // Only include 'page' when we have a token; otherwise let API start from first page.
                if let Some(tok) = &page_token {
                    body_map.insert("page".to_string(), tok.clone());
                } else {
                    body_map.remove("page");
                }
                body_map.insert("size".to_string(), serde_json::Value::Number(size.into()));
                let body = serde_json::Value::Object(body_map);

                tracing::trace!(
                    filter_idx,
                    page_num = fetched_pages + 1,
                    ?page_token,
                    size,
                    total_assets_found = all_entries.len(),
                    "Fetching Immich assets page"
                );

                let resp = client
                    .post(&url)
                    .header("x-api-key", self.cfg.api_key.clone().unwrap_or_default())
                    .json(&body)
                    .send()
                    .await
                    .context("immich search assets")?;

                if !resp.status().is_success() {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    tracing::warn!(%status, body=%text, "immich search assets failed for filter");
                    break; // stop paging for this filter
                }

                let items = resp.json::<serde_json::Value>().await.unwrap_or_default();
                let assets_obj = items.get("assets");
                let arr = assets_obj
                    .and_then(|v| v.get("items"))
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_else(|| items.as_array().cloned().unwrap_or_default());

                tracing::trace!(
                    filter_idx,
                    page_num = fetched_pages + 1,
                    assets_in_page = arr.len(),
                    "Received Immich assets page"
                );

                if arr.is_empty() {
                    tracing::trace!(
                        filter_idx,
                        page_num = fetched_pages + 1,
                        "No more assets in page, stopping pagination"
                    );
                    break; // no more pages
                }

                let mut new_assets_this_page = 0;

                for item in &arr {
                    let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    if id.is_empty() || seen_ids.contains(id) {
                        continue; // Skip duplicates
                    }
                    seen_ids.insert(id.to_string());

                    let exif = item.get("exifInfo");
                    let raw_w = exif
                        .and_then(|m| m.get("exifImageWidth"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    let raw_h = exif
                        .and_then(|m| m.get("exifImageHeight"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    let exif_orientation = exif
                        .and_then(|m| m.get("orientation"))
                        .and_then(|v| {
                            if let Some(n) = v.as_u64() {
                                Some(n)
                            } else if let Some(s) = v.as_str() {
                                s.trim().parse::<u64>().ok()
                            } else {
                                None
                            }
                        })
                        .unwrap_or(1);
                    let (w, h) = match exif_orientation {
                        6 | 8 => (raw_h, raw_w),
                        _ => (raw_w, raw_h),
                    };

                    let orient = if w > 0 && h > 0 {
                        Orientation::from_dims(w, h)
                    } else {
                        Orientation::Landscape
                    };
                    all_entries.push((id.to_string(), orient));
                    new_assets_this_page += 1;
                    total_assets_for_filter += 1;
                }

                tracing::debug!(
                    filter_idx,
                    page_num = fetched_pages + 1,
                    new_assets_this_page,
                    total_unique_assets = all_entries.len(),
                    total_assets_for_filter,
                    duplicates_skipped = arr.len() - new_assets_this_page,
                    "Processed Immich assets page"
                );

                fetched_pages += 1;
                // Advance using nextPage token when available; stop if null/missing.
                let next_page_val = assets_obj
                    .and_then(|v| v.get("nextPage"))
                    .cloned()
                    .or_else(|| items.get("nextPage").cloned());
                match next_page_val {
                    Some(v) if !v.is_null() => {
                        page_token = Some(v);
                        tracing::trace!(
                            filter_idx,
                            page_num = fetched_pages + 1,
                            ?page_token,
                            "Found next page token, continuing pagination"
                        );
                    }
                    _ => {
                        tracing::trace!(
                            filter_idx,
                            page_num = fetched_pages + 1,
                            "No next page token found, stopping pagination"
                        );
                        break;
                    }
                }
            }

            tracing::trace!(
                filter_idx,
                total_pages_fetched = fetched_pages,
                total_assets_for_filter,
                "Completed Immich search for filter"
            );
        }

        tracing::debug!(
            total_filters = searches.len(),
            total_unique_assets = all_entries.len(),
            "Completed all Immich metadata searches"
        );

        // Only update last_list timestamp on successful completion of all searches
        *self.entries.write() = all_entries;
        self.last_list
            .store(now, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
}

#[async_trait]
impl ImageSource for ImmichImageSource {
    async fn next(&self, desired: Orientation) -> Result<Option<ImageMeta>> {
        self.list_if_needed().await.ok();
        let snapshot: Vec<(String, Orientation)> = { self.entries.read().clone() };
        if snapshot.is_empty() {
            return Ok(None);
        }
        let order = self.cfg.order.unwrap_or_default();
        match order {
            OrderKind::Random => {
                for _ in 0..32 {
                    let idx = {
                        let mut rng = rng();
                        rng.random_range(0..snapshot.len())
                    };
                    let (asset_id, orient) = snapshot[idx].clone();
                    if orient != desired {
                        continue;
                    }
                    if let Some(meta) = self.fetch_asset(&asset_id, orient).await? {
                        return Ok(Some(meta));
                    }
                }
                Ok(None)
            }
            OrderKind::Sequential => {
                let total = snapshot.len();
                let start = self.cursor.fetch_add(1, AtomicOrdering::Relaxed);
                for offset in 0..total {
                    let idx = (start + offset) % total;
                    let (asset_id, orient) = &snapshot[idx];
                    if *orient != desired {
                        continue;
                    }
                    if let Some(meta) = self.fetch_asset(asset_id, *orient).await? {
                        if offset > 0 {
                            self.cursor.fetch_add(offset, AtomicOrdering::Relaxed);
                        }
                        return Ok(Some(meta));
                    }
                }
                Ok(None)
            }
        }
    }

    fn stats(&self) -> SourceStats {
        let g = self.entries.read();
        let metas: Vec<ImageMeta> = g
            .iter()
            .map(|(id, o)| ImageMeta {
                data: SourceData::Path(PathBuf::from("remote")),
                orientation: *o,
                date_taken: None, // Stats don't need actual date data
                exif_blob: None,  // Stats don't need EXIF data
                id: Some(id.clone()),
            })
            .collect();
        SourceStats::from_entries(&metas)
    }
}

impl ImmichImageSource {
    async fn fetch_asset(&self, asset_id: &str, orient: Orientation) -> Result<Option<ImageMeta>> {
        let client = reqwest::Client::new();
        let base = self.cfg.base_url.clone().unwrap_or_default();

        // Fetch thumbnail for image data
        let thumb_url = format!(
            "{}/api/assets/{}/thumbnail?size=preview",
            base.trim_end_matches('/'),
            asset_id
        );
        let thumb_resp = client
            .get(&thumb_url)
            .header("x-api-key", self.cfg.api_key.clone().unwrap_or_default())
            .send()
            .await?;

        if !thumb_resp.status().is_success() {
            return Ok(None);
        }

        let thumb_bytes = thumb_resp.bytes().await?;

        // Extract EXIF metadata from original asset (memory-efficient)
        let (date_taken, exif_blob) = self
            .extract_exif_metadata(asset_id)
            .await
            .unwrap_or((None, None));

        Ok(Some(ImageMeta {
            data: SourceData::Bytes(thumb_bytes.to_vec()),
            orientation: orient,
            date_taken,
            exif_blob,
            id: Some(asset_id.to_string()),
        }))
    }

    /// Memory-efficiently extract EXIF metadata from original asset without loading entire image
    async fn extract_exif_metadata(
        &self,
        asset_id: &str,
    ) -> Result<(Option<chrono::DateTime<chrono::Utc>>, Option<Vec<u8>>)> {
        let client = reqwest::Client::new();
        let base = self.cfg.base_url.clone().unwrap_or_default();
        let original_url = format!(
            "{}/api/assets/{}/original",
            base.trim_end_matches('/'),
            asset_id
        );

        let resp = client
            .get(&original_url)
            .header("x-api-key", self.cfg.api_key.clone().unwrap_or_default())
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok((None, None));
        }

        // Stream response and extract EXIF without loading entire image
        use std::io::Cursor;
        let bytes = resp.bytes().await?;
        let cursor = Cursor::new(&bytes[..]);

        // Use image crate's ImageReader to extract EXIF metadata efficiently
        let reader = image::ImageReader::new(cursor).with_guessed_format()?;
        let mut decoder = reader.into_decoder()?;

        // Extract EXIF metadata without decoding the image pixels
        if let Some(exif_bytes) = decoder.exif_metadata()? {
            // Parse EXIF to get DateTimeOriginal while preserving the full blob
            let exif_vec = exif_bytes.to_vec();
            let date_taken = crate::frame::extract_exif_date_taken_from_blob(&exif_vec)
                .ok()
                .flatten();
            return Ok((date_taken, Some(exif_vec)));
        }

        Ok((None, None))
    }
}

/// Factory creating concrete sources from config enum.
/// Factory creating a concrete boxed `ImageSource` from a typed config enum value.
pub fn build_source(src: &Source) -> Result<Box<dyn ImageSource>> {
    match src {
        Source::Filesystem { filesystem } => {
            let cfg = filesystem.clone().unwrap_or_default();
            Ok(Box::new(FilesystemImageSource::new(&cfg)?))
        }
        Source::Immich { immich } => {
            let cfg = immich.clone().unwrap_or_default();
            Ok(Box::new(ImmichImageSource::new(&cfg)?))
        }
        Source::Unknown => bail!("unknown source kind"),
    }
}
