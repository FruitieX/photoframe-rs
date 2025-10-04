use crate::config::{ImageLimits, OutputFormat, PhotoFrame, UploadTransport};
use crate::pipeline::{self, ProcessParams};
use crate::sources::{ImageMeta, SourceData};
use anyhow::{Context, Result};
use chrono::TimeZone;
use css_color::Srgb;
use image::ImageDecoder;
use image::ImageReader;
use image::metadata::Orientation;
use image::{DynamicImage, GenericImageView, RgbaImage};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::fs;
use tokio::sync::RwLock;
use tokio::time::sleep;

/// Represents an in-memory prepared frame image (currently just raw RGBA pixels).
pub struct PreparedFrameImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

// Global in-memory cache of last base (pre-adjustment) image per frame id.
static BASE_CACHE: OnceLock<RwLock<HashMap<String, DynamicImage>>> = OnceLock::new();

fn base_cache() -> &'static RwLock<HashMap<String, DynamicImage>> {
    BASE_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Read EXIF date_taken for a frame id from the persisted `<frame_id>_base.png`, if present.
pub async fn get_cached_date_taken(frame_id: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let path = PathBuf::from(format!("{frame_id}_base.png"));
    if !path.exists() {
        // Try intermediate as a fallback for older caches
        let ip = PathBuf::from(format!("{frame_id}_intermediate.png"));
        if ip.exists()
            && let Ok(bytes) = tokio::fs::read(&ip).await
            && let Ok(dt) = extract_exif_date_taken(&bytes)
        {
            return dt;
        }
        return None;
    }
    match tokio::fs::read(&path).await {
        Ok(bytes) => extract_exif_date_taken(&bytes).ok().flatten(),
        Err(_) => None,
    }
}

/// Load source image bytes and store base (unadjusted) image into cache & disk.
type LoadResult = (
    DynamicImage,
    Option<Orientation>,
    Option<chrono::DateTime<chrono::Utc>>,
    Option<Vec<u8>>,
);

pub async fn load_and_store_base(
    frame_id: &str,
    meta: &ImageMeta,
    _frame: &PhotoFrame,
    limits: Option<&ImageLimits>,
) -> Result<DynamicImage> {
    let (mut img, orientation_tag, mut date_taken, mut exif_blob): LoadResult = match &meta.data {
        SourceData::Path(p) => {
            let bytes = fs::read(p).await?;
            let tag = extract_exif_orientation(&bytes).ok().flatten();
            let date = extract_exif_date_taken(&bytes).ok().flatten();
            let exif = extract_exif_blob(&bytes).ok().flatten();
            (image::load_from_memory(&bytes)?, tag, date, exif)
        }
        SourceData::Bytes(b) => {
            let tag = extract_exif_orientation(b).ok().flatten();
            let date = extract_exif_date_taken(b).ok().flatten();
            let exif = extract_exif_blob(b).ok().flatten();
            (image::load_from_memory(b)?, tag, date, exif)
        }
    }; // original full-resolution

    // Prefer EXIF metadata from source (e.g., Immich original asset) over thumbnail EXIF
    if let Some(source_date) = meta.date_taken {
        date_taken = Some(source_date);
    }
    if let Some(source_exif) = &meta.exif_blob {
        exif_blob = Some(source_exif.clone());
    }

    if let Some(orient) = orientation_tag {
        img = apply_exif_orientation(img, orient);
    }
    img = downscale_to_limits(&img, limits);
    store_base(frame_id, &img, date_taken, exif_blob).await;
    store_metadata(frame_id, meta).await;
    Ok(img)
}

/// Attempt to read EXIF orientation using image crate decoder.
fn extract_exif_orientation(bytes: &[u8]) -> Result<Option<Orientation>> {
    use std::io::Cursor;
    let cursor = Cursor::new(bytes);
    let reader = ImageReader::new(cursor).with_guessed_format()?;
    let mut decoder = reader.into_decoder()?;
    Ok(decoder.orientation().ok())
}

/// Extract EXIF DateTimeOriginal/DateTime from raw EXIF blob.
pub fn extract_exif_date_taken_from_blob(
    exif_bytes: &[u8],
) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    let exif = exif::Reader::new().read_raw(exif_bytes.to_vec())?;

    // Helpers to retrieve ASCII values and to search all IFDs if PRIMARY is missing
    let get_ascii = |tag: exif::Tag| -> Option<String> {
        exif.get_field(tag, exif::In::PRIMARY)
            .or_else(|| exif.fields().find(|f| f.tag == tag))
            .and_then(|f| match &f.value {
                exif::Value::Ascii(v) if !v.is_empty() => std::str::from_utf8(&v[0])
                    .ok()
                    .map(|s| s.trim().to_string()),
                _ => None,
            })
    };

    // Build a base timestamp string from tags
    let mut base =
        match get_ascii(exif::Tag::DateTimeOriginal).or_else(|| get_ascii(exif::Tag::DateTime)) {
            Some(s) => s,
            None => return Ok(None),
        };

    // Append subseconds if present
    if let Some(sub) = get_ascii(exif::Tag::SubSecTimeOriginal)
        .or_else(|| get_ascii(exif::Tag::SubSecTime))
        .filter(|s| !s.is_empty())
    {
        base.push('.');
        base.push_str(sub.trim());
    }
    // Append normalized timezone offset if present
    if let Some(off_raw) =
        get_ascii(exif::Tag::OffsetTimeOriginal).or_else(|| get_ascii(exif::Tag::OffsetTime))
    {
        let mut off = off_raw.trim().replace(' ', "");
        // Normalize +HHMM -> +HH:MM
        if off.len() == 5
            && (off.starts_with('+') || off.starts_with('-'))
            && off.chars().skip(1).all(|c| c.is_ascii_digit())
        {
            off = format!("{}{}:{}", &off[0..2], &off[2..4], &off[4..5]);
        }
        // If already like +HH:MM, keep as-is
        if !off.is_empty() {
            base.push_str(off.as_str());
        }
    }

    // Try parsing with several formats
    if let Ok(dt) = chrono::DateTime::parse_from_str(&base, "%Y:%m:%d %H:%M:%S%.f%:z") {
        return Ok(Some(dt.with_timezone(&chrono::Utc)));
    }
    if let Ok(dt) = chrono::DateTime::parse_from_str(&base, "%Y:%m:%d %H:%M:%S%:z") {
        return Ok(Some(dt.with_timezone(&chrono::Utc)));
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(&base, "%Y:%m:%d %H:%M:%S%.f") {
        // Interpret as local time if no offset is present, then convert to UTC for storage
        let local = chrono::Local.from_local_datetime(&naive).earliest();
        return Ok(local.map(|ldt| ldt.with_timezone(&chrono::Utc)));
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(&base, "%Y:%m:%d %H:%M:%S") {
        let local = chrono::Local.from_local_datetime(&naive).earliest();
        return Ok(local.map(|ldt| ldt.with_timezone(&chrono::Utc)));
    }
    Ok(None)
}

/// Extract EXIF DateTimeOriginal/DateTime via image crate decoder.
fn extract_exif_date_taken(bytes: &[u8]) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    use std::io::Cursor;
    // First, try to get raw EXIF via image decoder
    let exif_opt: Option<exif::Exif> = (|| {
        let cursor = Cursor::new(bytes);
        let reader = ImageReader::new(cursor).with_guessed_format().ok()?;
        let mut decoder = reader.into_decoder().ok()?;
        let exif_bytes = decoder.exif_metadata().ok().flatten()?;
        exif::Reader::new().read_raw(exif_bytes).ok()
    })();

    // Fallback: ask kamadak-exif to read from the container (e.g., JPEG) directly
    let exif = match exif_opt {
        Some(r) => r,
        None => {
            let mut cur = Cursor::new(bytes);
            match exif::Reader::new().read_from_container(&mut cur) {
                Ok(r) => r,
                Err(_) => return Ok(None),
            }
        }
    };

    // Helpers to retrieve ASCII values and to search all IFDs if PRIMARY is missing
    let get_ascii = |tag: exif::Tag| -> Option<String> {
        exif.get_field(tag, exif::In::PRIMARY)
            .or_else(|| exif.fields().find(|f| f.tag == tag))
            .and_then(|f| match &f.value {
                exif::Value::Ascii(v) if !v.is_empty() => std::str::from_utf8(&v[0])
                    .ok()
                    .map(|s| s.trim().to_string()),
                _ => None,
            })
    };

    // Build a base timestamp string from tags
    let mut base =
        match get_ascii(exif::Tag::DateTimeOriginal).or_else(|| get_ascii(exif::Tag::DateTime)) {
            Some(s) => s,
            None => return Ok(None),
        };

    // Append subseconds if present
    if let Some(sub) = get_ascii(exif::Tag::SubSecTimeOriginal)
        .or_else(|| get_ascii(exif::Tag::SubSecTime))
        .filter(|s| !s.is_empty())
    {
        base.push('.');
        base.push_str(sub.trim());
    }
    // Append normalized timezone offset if present
    if let Some(off_raw) =
        get_ascii(exif::Tag::OffsetTimeOriginal).or_else(|| get_ascii(exif::Tag::OffsetTime))
    {
        let mut off = off_raw.trim().replace(' ', "");
        // Normalize +HHMM -> +HH:MM
        if off.len() == 5
            && (off.starts_with('+') || off.starts_with('-'))
            && off.chars().skip(1).all(|c| c.is_ascii_digit())
        {
            off = format!("{}{}:{}", &off[0..2], &off[2..4], &off[4..5]);
        }
        // If already like +HH:MM, keep as-is
        if !off.is_empty() {
            base.push_str(off.as_str());
        }
    }

    // Try parsing with several formats
    if let Ok(dt) = chrono::DateTime::parse_from_str(&base, "%Y:%m:%d %H:%M:%S%.f%:z") {
        return Ok(Some(dt.with_timezone(&chrono::Utc)));
    }
    if let Ok(dt) = chrono::DateTime::parse_from_str(&base, "%Y:%m:%d %H:%M:%S%:z") {
        return Ok(Some(dt.with_timezone(&chrono::Utc)));
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(&base, "%Y:%m:%d %H:%M:%S%.f") {
        // Interpret as local time if no offset is present, then convert to UTC for storage
        let local = chrono::Local.from_local_datetime(&naive).earliest();
        return Ok(local.map(|ldt| ldt.with_timezone(&chrono::Utc)));
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(&base, "%Y:%m:%d %H:%M:%S") {
        let local = chrono::Local.from_local_datetime(&naive).earliest();
        return Ok(local.map(|ldt| ldt.with_timezone(&chrono::Utc)));
    }
    Ok(None)
}

/// Extract raw EXIF blob to re-embed when saving intermediates.
fn extract_exif_blob(bytes: &[u8]) -> Result<Option<Vec<u8>>> {
    use std::io::Cursor;
    let cursor = Cursor::new(bytes);
    let reader = ImageReader::new(cursor).with_guessed_format()?;
    let mut decoder = reader.into_decoder()?;
    Ok(decoder.exif_metadata().ok().flatten())
}

/// Apply orientation transform producing a correctly oriented image in view coordinates.
fn apply_exif_orientation(mut img: DynamicImage, orient: Orientation) -> DynamicImage {
    img.apply_orientation(orient);
    img
}

async fn store_base(
    frame_id: &str,
    img: &DynamicImage,
    _date_taken: Option<chrono::DateTime<chrono::Utc>>,
    exif_blob: Option<Vec<u8>>,
) {
    // Keep an in-memory copy of the base image pixels for fast reuse within the same process.
    {
        let mut guard = base_cache().write().await;
        guard.insert(frame_id.to_string(), img.clone());
    }

    // Persist to `<frame_id>_base.png` and embed EXIF if available.
    use image::{ImageEncoder, codecs::png::PngEncoder};
    use std::fs::File;
    let path = PathBuf::from(format!("{frame_id}_base.png"));
    let rgba = img.to_rgba8();
    match File::create(&path) {
        Ok(mut f) => {
            let mut enc = PngEncoder::new(&mut f);
            if let Some(exif) = exif_blob
                && let Err(e) = enc.set_exif_metadata(exif)
            {
                tracing::warn!(frame=%frame_id, error=%e, "failed to set EXIF on base png");
            }
            if let Err(e) = enc.write_image(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                image::ExtendedColorType::Rgba8,
            ) {
                tracing::warn!(frame=%frame_id, error=%e, "failed to encode base png");
            }
        }
        Err(e) => tracing::warn!(frame=%frame_id, error=%e, "failed to create base png"),
    }
}

/// Save metadata about the fetched image as JSON.
async fn store_metadata(frame_id: &str, meta: &ImageMeta) {
    let path = PathBuf::from(format!("{frame_id}_metadata.json"));

    let filename_or_id = match &meta.data {
        SourceData::Path(p) => p.to_string_lossy().to_string(),
        SourceData::Bytes(_) => meta.id.clone().unwrap_or_else(|| "unknown".to_string()),
    };

    // Keep structure simple but informative. If we have Immich metadata already, store it raw.
    let mut root = serde_json::Map::new();
    root.insert(
        "source_id".to_string(),
        serde_json::Value::from(meta.source_id.clone()),
    );
    root.insert(
        "filename".to_string(),
        serde_json::Value::from(filename_or_id),
    );
    root.insert(
        "asset_id".to_string(),
        serde_json::Value::from(meta.id.clone()),
    );
    root.insert(
        "orientation".to_string(),
        serde_json::Value::from(format!("{:?}", meta.orientation)),
    );
    if let Some(dt) = meta.date_taken {
        root.insert(
            "date_taken".to_string(),
            serde_json::Value::from(dt.to_rfc3339()),
        );
    }
    if let Some(v) = &meta.asset_metadata {
        // Store under immich_metadata as requested
        root.insert("immich_metadata".to_string(), v.clone());
    }

    let doc = serde_json::Value::Object(root);
    if let Err(e) = tokio::fs::write(
        &path,
        serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".into()),
    )
    .await
    {
        tracing::warn!(frame=%frame_id, error=%e, "failed to write metadata json");
    }
}

/// Get a cloned base image from memory or disk.
pub async fn get_base_image(frame_id: &str) -> Result<Option<DynamicImage>> {
    if let Some(img) = base_cache().read().await.get(frame_id).cloned() {
        return Ok(Some(img));
    }
    let path = PathBuf::from(format!("{frame_id}_base.png"));
    if path.exists() {
        let img = image::open(&path)?;
        // populate cache for next time
        {
            let mut guard = base_cache().write().await;
            guard.insert(frame_id.to_string(), img.clone());
        }
        return Ok(Some(img));
    }
    Ok(None)
}

/// Produce a prepared image from a cached/stored base using current frame adjustments.
pub fn prepare_from_base(frame: &PhotoFrame, base: &DynamicImage) -> PreparedFrameImage {
    prepare_from_base_with_date(frame, base, None)
}

/// Produce a prepared image from a cached/stored base using current frame adjustments with date taken.
pub fn prepare_from_base_with_date(
    frame: &PhotoFrame,
    base: &DynamicImage,
    date_taken: Option<chrono::DateTime<chrono::Utc>>,
) -> PreparedFrameImage {
    let palette_vec = derive_palette(frame);

    let (w, h, pixels) = pipeline::process(ProcessParams {
        frame,
        base,
        palette: palette_vec.as_deref(),
        date_taken,
    })
    .expect("processing failed");

    PreparedFrameImage {
        width: w,
        height: h,
        pixels,
    }
}

/// Assume `scaled` is already scaled & padded to panel size; apply adjustments and dithering only.
pub fn prepare_from_scaled(frame: &PhotoFrame, scaled: &DynamicImage) -> PreparedFrameImage {
    let palette_vec = derive_palette(frame);

    let (w, h, pixels) = pipeline::process_from_scaled(ProcessParams {
        frame,
        base: scaled,
        palette: palette_vec.as_deref(),
        date_taken: None,
    })
    .expect("processing failed");

    PreparedFrameImage {
        width: w,
        height: h,
        pixels,
    }
}

/// Variant that allows passing a known date_taken for timestamp rendering.
pub fn prepare_from_scaled_with_date(
    frame: &PhotoFrame,
    scaled: &DynamicImage,
    date_taken: Option<chrono::DateTime<chrono::Utc>>,
) -> PreparedFrameImage {
    let palette_vec = derive_palette(frame);

    let (w, h, pixels) = pipeline::process_from_scaled(ProcessParams {
        frame,
        base: scaled,
        palette: palette_vec.as_deref(),
        date_taken,
    })
    .expect("processing failed");

    PreparedFrameImage {
        width: w,
        height: h,
        pixels,
    }
}

/// Derive a palette from supported_colors; returns None if list empty or only invalid entries.
fn derive_palette(frame: &PhotoFrame) -> Option<Vec<[u8; 3]>> {
    if frame.supported_colors.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for c in &frame.supported_colors {
        if let Ok(parsed) = c.parse::<Srgb>() {
            let r = (parsed.red * 255.0).round().clamp(0.0, 255.0) as u8;
            let g = (parsed.green * 255.0).round().clamp(0.0, 255.0) as u8;
            let b = (parsed.blue * 255.0).round().clamp(0.0, 255.0) as u8;
            tracing::trace!(input=%c, hex=format!("#{:02x}{:02x}{:02x}", r, g, b), r=%r, g=%g, b=%b, "resolved palette color");
            out.push([r, g, b]);
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

// Removed custom hex parser in favor of css-color crate.

/// Post a prepared image to the physical frame device.
pub async fn push_to_device(
    frame_id: &str,
    frame: &PhotoFrame,
    prepared: &PreparedFrameImage,
) -> Result<()> {
    // Determine rotation purely from native panel orientation and optional flip.
    // Prepared image is in view orientation: (vw,vh). Native device expects (panel_w,panel_h).
    // If a swap of dimensions is needed to match native, rotate by 270 (CCW) consistently.
    let mut rotation: u16 = 0;
    if let (Some(pw), Some(ph)) = (frame.panel_width, frame.panel_height) {
        let (vw, vh) = (prepared.width, prepared.height);
        let (native_w, native_h) = (pw, ph);
        if (vw, vh) != (native_w, native_h) && (vh, vw) == (native_w, native_h) {
            rotation = 270; // CCW to swap dimensions
        }
    }
    if frame.flip.unwrap_or(false) {
        rotation = ((rotation as u32 + 180) % 360) as u16;
    }

    // Apply final in-memory rotation (not persisted to preview files) just before upload.
    let (mut send_w, mut send_h, mut send_pixels) = if rotation == 0 {
        (prepared.width, prepared.height, prepared.pixels.clone())
    } else {
        let img =
            image::RgbaImage::from_raw(prepared.width, prepared.height, prepared.pixels.clone())
                .ok_or_else(|| anyhow::anyhow!("invalid pixel buffer for rotation"))?;
        let base_img = image::DynamicImage::ImageRgba8(img);
        let rotated: image::DynamicImage = match rotation {
            90 => image::DynamicImage::ImageRgba8(image::imageops::rotate90(&base_img)),
            180 => image::DynamicImage::ImageRgba8(image::imageops::rotate180(&base_img)),
            270 => image::DynamicImage::ImageRgba8(image::imageops::rotate270(&base_img)),
            other => {
                tracing::warn!(deg=%other, "unsupported rotation value; skipping");
                base_img
            }
        };
        let (w, h) = rotated.dimensions();
        (w, h, rotated.to_rgba8().into_raw())
    };

    // If native panel dims are provided and differ from current, pad/crop to match native canvas
    if let (Some(pw), Some(ph)) = (frame.panel_width, frame.panel_height)
        && (send_w, send_h) != (pw, ph)
    {
        let mut canvas = image::ImageBuffer::from_pixel(pw, ph, image::Rgba([255, 255, 255, 255]));
        if let Some(img) = image::RgbaImage::from_raw(send_w, send_h, send_pixels.clone()) {
            let dx = ((pw as i32 - send_w as i32) / 2).max(0) as i64;
            let dy = ((ph as i32 - send_h as i32) / 2).max(0) as i64;
            image::imageops::overlay(&mut canvas, &image::DynamicImage::ImageRgba8(img), dx, dy);
            send_w = pw;
            send_h = ph;
            send_pixels = image::DynamicImage::ImageRgba8(canvas)
                .to_rgba8()
                .into_raw();
        }
    }
    tracing::debug!(effective_rotation_deg=%rotation, flip=?frame.flip, view_w=%prepared.width, view_h=%prepared.height, native_w=?frame.panel_width, native_h=?frame.panel_height, send_w=%send_w, send_h=%send_h, "pushing frame with rotation");

    // Write the exact buffer that will be sent (after rotation) as PNG for debugging.
    if let Some(buf) = image::RgbaImage::from_raw(send_w, send_h, send_pixels.clone()) {
        let debug_img = image::DynamicImage::ImageRgba8(buf);
        let debug_path = std::path::PathBuf::from(format!("{frame_id}_sent.png"));
        if let Err(e) = debug_img.save(&debug_path) {
            tracing::warn!(frame=%frame_id, error=%e, "failed to save sent debug png");
        } else {
            tracing::debug!(frame=%frame_id, path=%debug_path.display(), "wrote sent debug png");
        }
    } else {
        tracing::warn!(frame=%frame_id, "invalid buffer when saving sent debug png");
    }

    // Encode per output format.
    let output_format = frame.output_format.unwrap_or(OutputFormat::Png);
    let (body_bytes, content_type): (Vec<u8>, &'static str) = match output_format {
        OutputFormat::Png => {
            let img_buf = image::RgbaImage::from_raw(send_w, send_h, send_pixels)
                .ok_or_else(|| anyhow::anyhow!("invalid pixel buffer for png"))?;
            let img_dyn = image::DynamicImage::ImageRgba8(img_buf);
            let mut bytes = Vec::new();
            img_dyn
                .write_to(
                    &mut std::io::Cursor::new(&mut bytes),
                    image::ImageFormat::Png,
                )
                .map_err(|e| anyhow::anyhow!("png encode failed: {e}"))?;
            (bytes, "image/png")
        }
        OutputFormat::Packed4bpp => {
            // If a palette is configured, map pixels to palette index (order = configured order).
            // Otherwise fallback to 16-level grayscale by luminance.
            let mut palette: Vec<[u8; 3]> = Vec::new();
            if !frame.supported_colors.is_empty() {
                for (i, c) in frame.supported_colors.iter().enumerate() {
                    if i >= 16 {
                        tracing::warn!(
                            "supported_colors has >16 entries, extra colors ignored for 4bpp"
                        );
                        break;
                    }
                    if let Ok(parsed) = c.parse::<Srgb>() {
                        let r = (parsed.red * 255.0).round().clamp(0.0, 255.0) as u8;
                        let g = (parsed.green * 255.0).round().clamp(0.0, 255.0) as u8;
                        let b = (parsed.blue * 255.0).round().clamp(0.0, 255.0) as u8;
                        palette.push([r, g, b]);
                    } else {
                        tracing::warn!(color=%c, "failed to parse supported_colors entry");
                    }
                }
            }

            // Map palette indices to device nibble codes based on nearest known device colors.
            // From GDEP040E01 reference: 0x0=Black, 0x1=White, 0x2=Yellow, 0x3=Red, 0x5=Blue, 0x6=Green.
            let idx_to_nibble: Option<Vec<u8>> = if !palette.is_empty() {
                let known: [([u8; 3], u8); 6] = [
                    ([0, 0, 0], 0x0),       // Black
                    ([255, 255, 255], 0x1), // White
                    ([255, 255, 0], 0x2),   // Yellow
                    ([255, 0, 0], 0x3),     // Red
                    ([0, 0, 255], 0x5),     // Blue
                    ([0, 255, 0], 0x6),     // Green
                ];
                let mut map = Vec::with_capacity(palette.len());
                for &p in &palette {
                    let mut best_n = known[0].1;
                    let mut best_d = u32::MAX;
                    for &(kc, nib) in &known {
                        let dr = p[0] as i32 - kc[0] as i32;
                        let dg = p[1] as i32 - kc[1] as i32;
                        let db = p[2] as i32 - kc[2] as i32;
                        let d = (dr * dr + dg * dg + db * db) as u32;
                        if d < best_d {
                            best_d = d;
                            best_n = nib;
                        }
                    }
                    map.push(best_n);
                }
                Some(map)
            } else {
                None
            };
            let mut out = Vec::with_capacity((send_w * send_h / 2) as usize);
            let mut nibble = None::<u8>;
            let reverse_rows = frame.reverse_rows.unwrap_or(false);
            let reverse_cols = frame.reverse_cols.unwrap_or(false);
            let swap_nibbles = frame.swap_nibbles.unwrap_or(false);
            let row_iter: Box<dyn Iterator<Item = u32>> = if reverse_rows {
                Box::new((0..send_h).rev())
            } else {
                Box::new(0..send_h)
            };
            for y in row_iter {
                let col_iter: Box<dyn Iterator<Item = u32>> = if reverse_cols {
                    Box::new((0..send_w).rev())
                } else {
                    Box::new(0..send_w)
                };
                for x in col_iter {
                    let idx = ((y as usize * send_w as usize) + x as usize) * 4;
                    let r = send_pixels[idx];
                    let g = send_pixels[idx + 1];
                    let b = send_pixels[idx + 2];
                    let val: u8 = if !palette.is_empty() {
                        let mut best_i = 0usize;
                        let mut best_dist = u32::MAX;
                        for (i, &p) in palette.iter().enumerate() {
                            if p[0] == r && p[1] == g && p[2] == b {
                                best_i = i;
                                break;
                            }
                            let dr = r as i32 - p[0] as i32;
                            let dg = g as i32 - p[1] as i32;
                            let db = b as i32 - p[2] as i32;
                            let dist = (dr * dr + dg * dg + db * db) as u32;
                            if dist < best_dist {
                                best_dist = dist;
                                best_i = i;
                            }
                        }
                        match &idx_to_nibble {
                            Some(m) if best_i < m.len() => m[best_i] & 0x0F,
                            _ => (best_i as u8) & 0x0F,
                        }
                    } else {
                        let l =
                            (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32).round() as u8;
                        ((l as u16 * 15 / 255) as u8) & 0x0F
                    };
                    if let Some(first) = nibble.take() {
                        if swap_nibbles {
                            out.push((val << 4) | first);
                        } else {
                            out.push((first << 4) | val);
                        }
                    } else {
                        nibble = Some(val);
                    }
                }
                if let Some(first) = nibble.take() {
                    if swap_nibbles {
                        out.push(first & 0x0F);
                    } else {
                        out.push(first << 4);
                    }
                }
            }
            (out, "application/octet-stream")
        }
    };

    if frame.dummy {
        tracing::info!(
            "[dummy] would push {} bytes to frame",
            prepared.pixels.len()
        );
        return Ok(());
    }

    let client = reqwest::Client::new();
    let url = &frame
        .upload_endpoint
        .clone()
        .context("missing upload_endpoint")?;
    let transport = frame.upload_transport.unwrap_or(UploadTransport::Raw);

    // Retry up to 5 times with exponential backoff starting at 20s.
    let max_attempts = 5u32;
    let mut delay = Duration::from_secs(20);
    for attempt in 1..=max_attempts {
        tracing::info!(frame=%frame_id, attempt=%attempt, url=%url, "pushing image to frame");

        let send_result: anyhow::Result<reqwest::Response> = match transport {
            UploadTransport::Raw => {
                let bytes = body_bytes.clone();
                client
                    .post(url)
                    .header(reqwest::header::CONTENT_TYPE, content_type)
                    .body(bytes)
                    .send()
                    .await
                    .map_err(|e| e.into())
            }
            UploadTransport::Multipart => {
                let bytes = body_bytes.clone();
                let part = reqwest::multipart::Part::bytes(bytes)
                    .file_name(match output_format {
                        OutputFormat::Png => "image.png",
                        OutputFormat::Packed4bpp => "image.bin",
                    })
                    .mime_str(content_type)
                    .map_err(|e| anyhow::anyhow!("invalid mime '{}': {e}", content_type))?;
                let form = reqwest::multipart::Form::new().part("file", part);
                client
                    .post(url)
                    .multipart(form)
                    .send()
                    .await
                    .map_err(|e| e.into())
            }
        };

        match send_result {
            Ok(resp) => {
                if resp.status().is_success() {
                    tracing::info!(frame=%frame_id, status=%resp.status().as_u16(), "push succeeded");
                    return Ok(());
                }
                let status = resp.status();
                if attempt >= max_attempts {
                    anyhow::bail!(
                        "device responded with status {} after {} attempts",
                        status,
                        attempt
                    );
                } else {
                    tracing::warn!(frame=%frame_id, attempt=%attempt, status=%status.as_u16(), wait_secs=%delay.as_secs(), "device responded with non-success; retrying");
                }
            }
            Err(e) => {
                if attempt >= max_attempts {
                    return Err(e)
                        .with_context(|| format!("upload failed after {} attempts", attempt));
                } else {
                    tracing::warn!(frame=%frame_id, attempt=%attempt, error=%e, wait_secs=%delay.as_secs(), "upload error; retrying");
                }
            }
        }

        sleep(delay).await;
        delay = delay.saturating_mul(2);
    }

    // Should be unreachable due to returns inside the loop
    anyhow::bail!("upload failed")
}

/// Convenience: full pipeline from source metadata to pushing to device.
pub async fn process_and_push(
    frame_id: &str,
    frame: &PhotoFrame,
    meta: &ImageMeta,
    limits: Option<&ImageLimits>,
) -> Result<()> {
    let base = load_and_store_base(frame_id, meta, frame, limits).await?;

    // Compute scaled once and reuse for both saving and final processing.
    let scaled = pipeline::scale_and_pad_only(frame, &base);

    // Save intermediate (pre-dither) snapshot with date taken metadata
    let date_taken = get_cached_date_taken(frame_id).await;
    if let Err(e) = save_intermediate_scaled_with_metadata(frame_id, &scaled, date_taken).await {
        tracing::warn!(frame=%frame_id, error=%e, "failed saving intermediate image");
    }
    let prepared = prepare_from_scaled_with_date(frame, &scaled, date_taken);
    let _path = save_prepared(frame_id, &prepared)?; // ignore path for now
    push_to_device(frame_id, frame, &prepared).await?;
    Ok(())
}

/// Handle a direct user-uploaded image (bytes) for a frame.
pub async fn handle_direct_upload(
    frame_id: &str,
    frame: &PhotoFrame,
    bytes: &[u8],
    limits: Option<&ImageLimits>,
) -> Result<PreparedFrameImage> {
    let mut img = image::load_from_memory(bytes)?;
    let date_taken = extract_exif_date_taken(bytes).ok().flatten();
    img = downscale_to_limits(&img, limits);
    let exif_blob = extract_exif_blob(bytes).ok().flatten();
    store_base(frame_id, &img, date_taken, exif_blob).await; // persist unadjusted base before modifications

    // Compute & save intermediate once, then finish from scaled
    let scaled = pipeline::scale_and_pad_only(frame, &img);
    if let Err(e) = save_intermediate_scaled_with_metadata(frame_id, &scaled, date_taken).await {
        tracing::warn!(frame=%frame_id, error=%e, "failed saving intermediate image (upload)");
    }
    let prepared = prepare_from_scaled_with_date(frame, &scaled, date_taken);
    Ok(prepared)
}

fn downscale_to_limits(img: &DynamicImage, limits: Option<&ImageLimits>) -> DynamicImage {
    let Some(l) = limits else {
        return img.clone();
    };
    if l.max_width.is_none() && l.max_height.is_none() {
        return img.clone();
    }
    let (w, h) = img.dimensions();
    let mw = l.max_width.unwrap_or(w);
    let mh = l.max_height.unwrap_or(h);
    if w <= mw && h <= mh {
        return img.clone();
    }
    // image::DynamicImage::resize preserves aspect ratio and fits inside the box.
    let resized = img.resize(mw, mh, image::imageops::FilterType::CatmullRom);
    DynamicImage::ImageRgba8(resized.to_rgba8())
}

/// Save prepared image to working directory as `<frame_id>.png`.
pub fn save_prepared(frame_id: &str, prepared: &PreparedFrameImage) -> Result<PathBuf> {
    let path = PathBuf::from(format!("{frame_id}.png"));
    let buf = RgbaImage::from_raw(prepared.width, prepared.height, prepared.pixels.clone())
        .ok_or_else(|| anyhow::anyhow!("invalid pixel buffer size"))?;
    let dynimg = DynamicImage::ImageRgba8(buf);
    dynimg.save(&path)?;
    Ok(path)
}

/// Save a pre-dither intermediate image (after scaling/overscan and adjustments) as `<frame_id>_intermediate.png`.
pub async fn save_intermediate_from_base(
    frame_id: &str,
    frame: &PhotoFrame,
    base: &DynamicImage,
) -> Result<PathBuf> {
    // Intermediate is generated post scaling/padding but pre adjustments.
    let img = pipeline::scale_and_pad_only(frame, base);
    // Delegate to the EXIF-aware writer to preserve EXIF from base PNG.
    save_intermediate_scaled_with_metadata(frame_id, &img, None).await
}

/// Save a pre-dither intermediate image (after scaling/overscan and adjustments) from a prepared scaled image.
pub async fn save_intermediate_scaled(frame_id: &str, scaled: &DynamicImage) -> Result<PathBuf> {
    save_intermediate_scaled_with_metadata(frame_id, scaled, None).await
}

pub async fn save_intermediate_scaled_with_metadata(
    frame_id: &str,
    scaled: &DynamicImage,
    _date_taken: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<PathBuf> {
    use image::{ImageEncoder, codecs::png::PngEncoder};
    use std::fs::File;
    let path = PathBuf::from(format!("{frame_id}_intermediate.png"));
    let rgba = scaled.to_rgba8();
    let mut f = File::create(&path).with_context(|| format!("create {}", path.display()))?;
    let mut enc = PngEncoder::new(&mut f);
    // Attempt to copy EXIF from the persisted base PNG so metadata survives in the preview.
    if let Some(exif) = read_exif_from_base_png(frame_id).await {
        let _ = enc.set_exif_metadata(exif);
    }
    enc.write_image(
        rgba.as_raw(),
        rgba.width(),
        rgba.height(),
        image::ExtendedColorType::Rgba8,
    )
    .with_context(|| format!("encode {}", path.display()))?;
    Ok(path)
}

/// Read raw EXIF blob from `<frame_id>_base.png`, if any.
async fn read_exif_from_base_png(frame_id: &str) -> Option<Vec<u8>> {
    use std::io::Cursor;
    let path = PathBuf::from(format!("{frame_id}_base.png"));
    let bytes = tokio::fs::read(&path).await.ok()?;
    let cursor = Cursor::new(bytes);
    let reader = ImageReader::new(cursor).with_guessed_format().ok()?;
    let mut decoder = reader.into_decoder().ok()?;
    decoder.exif_metadata().ok().flatten()
}
