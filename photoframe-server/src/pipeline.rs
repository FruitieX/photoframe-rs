use crate::config::{Adjustments, PhotoFrame, ScalingMode};
use crate::dither::dither_image;
use crate::timestamp::render_timestamp;
use anyhow::Result;
use image::imageops;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};

/// Full processing context.
pub struct ProcessParams<'a> {
    pub frame: &'a PhotoFrame,
    pub base: &'a DynamicImage,
    pub palette: Option<&'a [[u8; 3]]>,
    pub date_taken: Option<chrono::NaiveDateTime>,
}

/// Run full pipeline from base to prepared RGBA pixel vec.
pub fn process(params: ProcessParams) -> Result<(u32, u32, Vec<u8>)> {
    let frame = params.frame;
    let mut img = params.base.clone();

    // Determine if we need to reduce image area for full-width banner.
    // Only reduce if timestamp is enabled AND we actually have a date to render.
    let reduced_height = if let (Some(ts), Some(_)) = (&frame.timestamp, params.date_taken) {
        if ts.enabled && ts.full_width_banner {
            calculate_reduced_height_for_banner(frame, ts)
        } else {
            None
        }
    } else {
        None
    };

    // 1) Scale + pad to panel first, and capture content rect
    let (mut composed, (cx, cy, cw, ch)) =
        scale_and_pad_with_rect_reduced(frame, &img, reduced_height);
    // 2) Apply adjustments only on the content rect (exclude white padding)
    if let Some(adj) = frame.adjustments.as_ref() {
        let mut full = composed.to_rgba8();
        // Guard against zero-size
        if cw > 0 && ch > 0 && cx + cw <= full.width() && cy + ch <= full.height() {
            // Extract subimage view, clone to a standalone buffer for in-place ops
            let view = image::imageops::crop(&mut full, cx, cy, cw, ch).to_image();
            let adjusted = apply_adjustments_fast(DynamicImage::ImageRgba8(view), Some(adj));
            // Paste adjusted back
            image::imageops::replace(&mut full, &adjusted.to_rgba8(), cx as i64, cy as i64);
            composed = DynamicImage::ImageRgba8(full);
        }
    }
    img = composed;

    // 3) Add timestamp if enabled and we have a date (render_timestamp will early-return otherwise)
    if let Some(ts) = &frame.timestamp
        && ts.enabled
    {
        img = render_timestamp(
            img,
            ts,
            reduced_height,
            params.date_taken,
            frame.overscan.as_ref(),
        )?;
    }

    // 4) Dither/palette reduce if requested
    if let Some(pal) = params.palette {
        let (w, h) = img.dimensions();
        let mut raw = img.to_rgba8().into_raw();
        dither_image(&mut raw, w, h, pal, frame.dithering.as_deref());
        return Ok((w, h, raw));
    }
    let (w, h) = img.dimensions();
    Ok((w, h, img.to_rgba8().into_raw()))
}

/// Variant of process that assumes `base` is already scaled/padded to panel; apply only
/// adjustments and then dithering/palette mapping.
pub fn process_from_scaled(params: ProcessParams) -> Result<(u32, u32, Vec<u8>)> {
    let frame = params.frame;
    let mut img = params.base.clone();
    img = apply_adjustments_fast(img, frame.adjustments.as_ref());

    // Add timestamp if enabled (note: for scaled input, we don't handle banner mode reduction)
    if let Some(ts) = &frame.timestamp
        && ts.enabled
    {
        img = render_timestamp(img, ts, None, params.date_taken, frame.overscan.as_ref())?;
    }

    if let Some(pal) = params.palette {
        let (w, h) = img.dimensions();
        let mut raw = img.to_rgba8().into_raw();
        dither_image(&mut raw, w, h, pal, frame.dithering.as_deref());
        return Ok((w, h, raw));
    }
    let (w, h) = img.dimensions();
    Ok((w, h, img.to_rgba8().into_raw()))
}

// (moved below) scale_and_pad_only now delegates to scale_and_pad_with_rect

pub(crate) fn apply_adjustments_fast(img: DynamicImage, adj: Option<&Adjustments>) -> DynamicImage {
    let Some(a) = adj else { return img };
    let mut buf = img.to_rgba8();
    // dimensions captured implicitly by buf.width()/height() as needed
    // Precompute coefficients
    let b_off: f32 = a.brightness.clamp(-255.0, 255.0);
    // Contrast using common formula mapped from [-50,50] to [-255,255] domain if needed.
    let c = a.contrast.clamp(-255.0, 255.0);
    let cf = if c.abs() < 0.01 {
        1.0
    } else {
        (259.0 * (c + 255.0)) / (255.0 * (259.0 - c))
    };
    // Saturation amount in [-1,1] roughly: assume input saturation range [-0.25,0.25] per UI, scale to [-1,1]
    let s = (a.saturation * 4.0).clamp(-1.0, 1.0);
    for px in buf.pixels_mut() {
        let r = px[0] as f32;
        let g = px[1] as f32;
        let b = px[2] as f32;
        // brightness
        let mut r1 = r + b_off;
        let mut g1 = g + b_off;
        let mut b1 = b + b_off;
        // contrast around 128
        r1 = (r1 - 128.0) * cf + 128.0;
        g1 = (g1 - 128.0) * cf + 128.0;
        b1 = (b1 - 128.0) * cf + 128.0;
        // saturation via luma mix
        if s.abs() > 0.001 {
            let l = 0.299 * r1 + 0.587 * g1 + 0.114 * b1;
            r1 = l + (r1 - l) * (1.0 + s);
            g1 = l + (g1 - l) * (1.0 + s);
            b1 = l + (b1 - l) * (1.0 + s);
        }
        px[0] = r1.clamp(0.0, 255.0) as u8;
        px[1] = g1.clamp(0.0, 255.0) as u8;
        px[2] = b1.clamp(0.0, 255.0) as u8;
        // alpha preserved
    }
    let mut out = DynamicImage::ImageRgba8(buf);
    // Sharpen/soften
    if a.sharpness.abs() >= 0.01 {
        let clamped = a.sharpness.clamp(-5.0, 5.0);
        let amt = (clamped.abs() / 5.0).clamp(0.0, 1.0);
        if amt > 0.0 {
            let sigma = 0.8 + amt * 1.6;
            if clamped > 0.0 {
                // Unsharp mask with threshold tuned for speed/quality
                out = image::DynamicImage::ImageRgba8(imageops::unsharpen(&out, sigma, 1));
            } else {
                out = image::DynamicImage::ImageRgba8(imageops::blur(&out, sigma));
            }
        }
    }
    out
}

/// Compute scaled+pad composition and return the final image plus the absolute content rect.
/// Content rect is the position and size of the resized image inside the full panel canvas.
pub fn scale_and_pad_with_rect(
    frame: &PhotoFrame,
    base: &DynamicImage,
) -> (DynamicImage, (u32, u32, u32, u32)) {
    let Some(panel_w) = frame.panel_width else {
        return (base.clone(), (0, 0, base.width(), base.height()));
    };
    let Some(panel_h) = frame.panel_height else {
        return (base.clone(), (0, 0, base.width(), base.height()));
    };
    // Derive "view canvas" from native panel dims and configured orientation.
    // Landscape => wide (max,min), Portrait => tall (min,max)
    let orient = frame.orientation.unwrap_or_default();
    let (view_w, view_h) = match orient {
        crate::config::Orientation::Landscape => (panel_w.max(panel_h), panel_w.min(panel_h)),
        crate::config::Orientation::Portrait => (panel_w.min(panel_h), panel_w.max(panel_h)),
    };
    let os = frame.overscan.clone().unwrap_or_default();
    let pad_left = os.left.max(0) as u32;
    let pad_right = os.right.max(0) as u32;
    let pad_top = os.top.max(0) as u32;
    let pad_bottom = os.bottom.max(0) as u32;
    // Overscan is in view coordinates
    let inner_w = view_w.saturating_sub(pad_left + pad_right).max(1);
    let inner_h = view_h.saturating_sub(pad_top + pad_bottom).max(1);
    let resized: DynamicImage = match frame.scaling.unwrap_or_default() {
        ScalingMode::Contain => base.resize(inner_w, inner_h, FilterType::Triangle),
        ScalingMode::Cover => base.resize_to_fill(inner_w, inner_h, FilterType::Triangle),
    };
    let mut inner_canvas: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(inner_w, inner_h, Rgba([255, 255, 255, 255]));
    let off_x = ((inner_w as i32 - resized.width() as i32) / 2).max(0) as u32;
    let off_y = ((inner_h as i32 - resized.height() as i32) / 2).max(0) as u32;
    image::imageops::overlay(&mut inner_canvas, &resized, off_x as i64, off_y as i64);
    let mut final_canvas: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(view_w, view_h, Rgba([255, 255, 255, 255]));
    image::imageops::overlay(
        &mut final_canvas,
        &DynamicImage::ImageRgba8(inner_canvas.clone()),
        pad_left as i64,
        pad_top as i64,
    );
    let content_x = pad_left + off_x;
    let content_y = pad_top + off_y;
    let content_w = resized.width();
    let content_h = resized.height();
    (
        DynamicImage::ImageRgba8(final_canvas),
        (content_x, content_y, content_w, content_h),
    )
}

/// Keep legacy helper signature but ignore the rect.
pub fn scale_and_pad_only(frame: &PhotoFrame, base: &DynamicImage) -> DynamicImage {
    let (img, _) = scale_and_pad_with_rect(frame, base);
    img
}

/// Calculate reduced height for banner mode.
fn calculate_reduced_height_for_banner(
    frame: &PhotoFrame,
    timestamp: &crate::config::Timestamp,
) -> Option<u32> {
    let panel_h = frame.panel_height?;
    let font_size = timestamp.font_size.unwrap_or(24.0);
    let padding = 8u32;
    let banner_height = timestamp
        .banner_height
        .unwrap_or(font_size as u32 + (padding * 2));

    Some(panel_h.saturating_sub(banner_height))
}

/// Modified scale_and_pad_with_rect that supports reduced height for banner mode.
pub fn scale_and_pad_with_rect_reduced(
    frame: &PhotoFrame,
    base: &DynamicImage,
    reduced_height: Option<u32>,
) -> (DynamicImage, (u32, u32, u32, u32)) {
    if reduced_height.is_some() {
        scale_and_pad_with_rect_internal(frame, base, reduced_height)
    } else {
        scale_and_pad_with_rect(frame, base)
    }
}

/// Internal function that handles both normal and reduced height scaling.
fn scale_and_pad_with_rect_internal(
    frame: &PhotoFrame,
    base: &DynamicImage,
    reduced_height: Option<u32>,
) -> (DynamicImage, (u32, u32, u32, u32)) {
    let Some(panel_w) = frame.panel_width else {
        return (base.clone(), (0, 0, base.width(), base.height()));
    };
    let Some(panel_h) = frame.panel_height else {
        return (base.clone(), (0, 0, base.width(), base.height()));
    };

    // Use reduced height if provided, otherwise use full panel height
    let effective_panel_h = reduced_height.unwrap_or(panel_h);

    // Derive "view canvas" from native panel dims and configured orientation.
    // Landscape => wide (max,min), Portrait => tall (min,max)
    let orient = frame.orientation.unwrap_or_default();
    let (view_w, view_h) = match orient {
        crate::config::Orientation::Landscape => (
            panel_w.max(effective_panel_h),
            panel_w.min(effective_panel_h),
        ),
        crate::config::Orientation::Portrait => (
            panel_w.min(effective_panel_h),
            panel_w.max(effective_panel_h),
        ),
    };
    let os = frame.overscan.clone().unwrap_or_default();
    let pad_left = os.left.max(0) as u32;
    let pad_right = os.right.max(0) as u32;
    let pad_top = os.top.max(0) as u32;
    let pad_bottom = os.bottom.max(0) as u32;
    // Overscan is in view coordinates
    let inner_w = view_w.saturating_sub(pad_left + pad_right).max(1);
    let inner_h = view_h.saturating_sub(pad_top + pad_bottom).max(1);
    let resized: DynamicImage = match frame.scaling.unwrap_or_default() {
        ScalingMode::Contain => base.resize(inner_w, inner_h, FilterType::Triangle),
        ScalingMode::Cover => base.resize_to_fill(inner_w, inner_h, FilterType::Triangle),
    };
    let mut inner_canvas: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(inner_w, inner_h, Rgba([255, 255, 255, 255]));
    let off_x = ((inner_w as i32 - resized.width() as i32) / 2).max(0) as u32;
    let off_y = ((inner_h as i32 - resized.height() as i32) / 2).max(0) as u32;
    image::imageops::overlay(&mut inner_canvas, &resized, off_x as i64, off_y as i64);
    let mut final_canvas: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(view_w, view_h, Rgba([255, 255, 255, 255]));
    image::imageops::overlay(
        &mut final_canvas,
        &DynamicImage::ImageRgba8(inner_canvas.clone()),
        pad_left as i64,
        pad_top as i64,
    );
    let content_x = pad_left + off_x;
    let content_y = pad_top + off_y;
    let content_w = resized.width();
    let content_h = resized.height();
    (
        DynamicImage::ImageRgba8(final_canvas),
        (content_x, content_y, content_w, content_h),
    )
}
