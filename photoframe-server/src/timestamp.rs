use crate::config::{Overscan, Timestamp, TimestampColor, TimestampPosition, TimestampStrokeColor};
use ab_glyph::{Font, FontRef, Glyph, OutlinedGlyph, PxScale, ScaleFont, point};
use anyhow::{Context, Result};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba, RgbaImage, imageops};

const DEFAULT_FONT_DATA: &[u8] = include_bytes!("../assets/fonts/DejaVuSans.ttf");

fn get_pixel_checked(img: &RgbaImage, x: u32, y: u32) -> Option<Rgba<u8>> {
    if x < img.width() && y < img.height() {
        Some(*img.get_pixel(x, y))
    } else {
        None
    }
}

fn get_pixel_mut_checked(img: &mut RgbaImage, x: u32, y: u32) -> Option<&mut Rgba<u8>> {
    if x < img.width() && y < img.height() {
        Some(img.get_pixel_mut(x, y))
    } else {
        None
    }
}

/// Lays out `text` on a single line with the baseline at y = 0, applying pair
/// kerning between consecutive glyphs. Returns the glyphs and the total advance width.
fn layout_line(font: &FontRef, scale: PxScale, text: &str) -> (Vec<Glyph>, f32) {
    let scaled = font.as_scaled(scale);
    let mut caret = 0.0f32;
    let mut last = None;
    let glyphs = text
        .chars()
        .map(|c| {
            let id = scaled.glyph_id(c);
            if let Some(prev) = last {
                caret += scaled.kern(prev, id);
            }
            let glyph = id.with_scale_and_position(scale, point(caret, 0.0));
            caret += scaled.h_advance(id);
            last = Some(id);
            glyph
        })
        .collect();
    (glyphs, caret)
}

/// Height of a line of text (ascent minus descent) at the given scale.
fn line_height(font: &FontRef, scale: PxScale) -> f32 {
    let scaled = font.as_scaled(scale);
    scaled.ascent() - scaled.descent()
}

struct AutoColorParams<'a> {
    canvas: &'a RgbaImage,
    position: TimestampPosition,
    scale: PxScale,
    font: &'a FontRef<'a>,
    text: &'a str,
    img_width: u32,
    img_height: u32,
    overscan: Option<&'a Overscan>,
    padding_horizontal: u32,
    padding_vertical: u32,
}

fn determine_auto_text_color(p: AutoColorParams) -> Rgba<u8> {
    let (_, width) = layout_line(p.font, p.scale, p.text);
    let text_width = width as u32;
    let text_height = line_height(p.font, p.scale) as u32;

    let (text_x, text_y) = calculate_text_position(&LayoutArea {
        position: p.position,
        text_width,
        text_height,
        area_width: p.img_width,
        area_height: p.img_height,
        area_y_offset: 0,
        overscan: p.overscan,
        padding_horizontal: p.padding_horizontal,
        padding_vertical: p.padding_vertical,
    });

    let mut total_brightness = 0u64;
    let mut pixel_count = 0u64;
    let sample_padding = 5;
    let sample_x_start = text_x.saturating_sub(sample_padding);
    let sample_x_end = (text_x + text_width + sample_padding).min(p.img_width);
    let sample_y_start = text_y.saturating_sub(sample_padding);
    let sample_y_end = (text_y + text_height + sample_padding).min(p.img_height);

    for y in sample_y_start..sample_y_end {
        for x in sample_x_start..sample_x_end {
            if let Some(pixel) = get_pixel_checked(p.canvas, x, y) {
                let luminance = (0.299 * pixel[0] as f32
                    + 0.587 * pixel[1] as f32
                    + 0.114 * pixel[2] as f32) as u64;
                total_brightness += luminance;
                pixel_count += 1;
            }
        }
    }

    if pixel_count == 0 {
        return Rgba([0, 0, 0, 255]);
    }
    let average_brightness = total_brightness / pixel_count;
    if average_brightness > 128 {
        Rgba([0, 0, 0, 255])
    } else {
        Rgba([255, 255, 255, 255])
    }
}

fn resolve_stroke(cfg: &Timestamp, fill: Rgba<u8>) -> (bool, u32, Rgba<u8>) {
    let enabled = cfg.stroke_enabled;
    let mut width = cfg.stroke_width.unwrap_or(1);
    if let Some(sz) = cfg.font_size {
        let max_rel = (sz * 0.3).round().clamp(1.0, 16.0) as u32;
        width = width.min(max_rel);
    }
    width = width.min(16);
    let stroke_color = match cfg.stroke_color.unwrap_or(TimestampStrokeColor::Auto) {
        TimestampStrokeColor::Auto => {
            let lum =
                (0.299 * fill[0] as f32 + 0.587 * fill[1] as f32 + 0.114 * fill[2] as f32) as u8;
            if lum > 128 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        }
        TimestampStrokeColor::White => Rgba([255, 255, 255, 255]),
        TimestampStrokeColor::Black => Rgba([0, 0, 0, 255]),
    };
    (enabled, width, stroke_color)
}

struct LayoutArea<'a> {
    position: TimestampPosition,
    text_width: u32,
    text_height: u32,
    area_width: u32,
    area_height: u32,
    area_y_offset: u32,
    overscan: Option<&'a Overscan>,
    padding_horizontal: u32,
    padding_vertical: u32,
}

fn calculate_text_position(p: &LayoutArea) -> (u32, u32) {
    let LayoutArea {
        position,
        text_width,
        text_height,
        area_width,
        area_height,
        area_y_offset,
        overscan,
        padding_horizontal,
        padding_vertical,
    } = *p;
    let default_overscan = Overscan::default();
    let overscan = overscan.unwrap_or(&default_overscan);
    let pad_left = overscan.left.max(0) as u32;
    let pad_right = overscan.right.max(0) as u32;
    let pad_top = overscan.top.max(0) as u32;
    let pad_bottom = overscan.bottom.max(0) as u32;

    let effective_width = area_width.saturating_sub(pad_left + pad_right);
    let effective_height = area_height.saturating_sub(pad_top + pad_bottom);

    let x = match position {
        TimestampPosition::TopLeft | TimestampPosition::BottomLeft => pad_left + padding_horizontal,
        TimestampPosition::TopCenter | TimestampPosition::BottomCenter => {
            pad_left + (effective_width.saturating_sub(text_width)) / 2
        }
        TimestampPosition::TopRight | TimestampPosition::BottomRight => {
            pad_left + effective_width.saturating_sub(text_width + padding_horizontal)
        }
    };

    let y = match position {
        TimestampPosition::TopLeft | TimestampPosition::TopCenter | TimestampPosition::TopRight => {
            area_y_offset + pad_top + padding_vertical
        }
        TimestampPosition::BottomLeft
        | TimestampPosition::BottomCenter
        | TimestampPosition::BottomRight => {
            area_y_offset
                + pad_top
                + effective_height.saturating_sub(text_height + padding_vertical)
        }
    };

    (x, y)
}

struct TextDrawParams<'a> {
    font: &'a FontRef<'a>,
    text: &'a str,
    scale: PxScale,
    position: TimestampPosition,
    color: Rgba<u8>,
    area_y: u32,
    area_height: u32,
    area_width: u32,
    overscan: Option<&'a Overscan>,
    padding_horizontal: u32,
    padding_vertical: u32,
    stroke_enabled: bool,
    stroke_width: u32,
    stroke_color: Rgba<u8>,
}

fn render_text_on_canvas(canvas: &mut RgbaImage, p: &TextDrawParams) -> Result<()> {
    let (glyphs, width) = layout_line(p.font, p.scale, p.text);
    if glyphs.is_empty() {
        return Ok(());
    }
    let outlined: Vec<OutlinedGlyph> = glyphs
        .into_iter()
        .filter_map(|g| p.font.outline_glyph(g))
        .collect();

    let text_width = width.ceil() as u32;
    let text_height = line_height(p.font, p.scale).ceil() as u32;

    // Compute layout with baseline-aware vertical placement to avoid clamping effects.
    let default_overscan = Overscan::default();
    let osc = p.overscan.unwrap_or(&default_overscan);
    let pad_left = osc.left.max(0) as u32;
    let pad_right = osc.right.max(0) as u32;
    let pad_top = osc.top.max(0) as u32;
    let pad_bottom = osc.bottom.max(0) as u32;

    let effective_width = p.area_width.saturating_sub(pad_left + pad_right);
    let effective_height = p.area_height.saturating_sub(pad_top + pad_bottom);

    // Horizontal position
    let x = match p.position {
        TimestampPosition::TopLeft | TimestampPosition::BottomLeft => {
            pad_left + p.padding_horizontal
        }
        TimestampPosition::TopCenter | TimestampPosition::BottomCenter => {
            pad_left + (effective_width.saturating_sub(text_width)) / 2
        }
        TimestampPosition::TopRight | TimestampPosition::BottomRight => {
            pad_left + effective_width.saturating_sub(text_width + p.padding_horizontal)
        }
    };

    // Baseline position: integer pixels
    let ascent = p.font.as_scaled(p.scale).ascent().ceil() as i32;
    let text_height_i = text_height as i32;
    let area_y_i = p.area_y as i32;
    let pad_top_i = pad_top as i32;
    let eff_h_i = effective_height as i32;
    let pad_v_i = p.padding_vertical as i32;

    let y_base: i32 = match p.position {
        TimestampPosition::TopLeft | TimestampPosition::TopCenter | TimestampPosition::TopRight => {
            area_y_i + pad_top_i + pad_v_i + ascent
        }
        TimestampPosition::BottomLeft
        | TimestampPosition::BottomCenter
        | TimestampPosition::BottomRight => {
            // Bottom edge (in layout coordinates) minus (text_height - ascent) minus padding
            area_y_i + pad_top_i + (eff_h_i - pad_v_i) - (text_height_i - ascent)
        }
    };

    // Stroke pass
    if p.stroke_enabled && p.stroke_width > 0 {
        let r = p.stroke_width as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx == 0 && dy == 0 {
                    continue;
                }
                if dx * dx + dy * dy > r * r {
                    continue;
                }
                for glyph in outlined.iter() {
                    let bbox = glyph.px_bounds();
                    glyph.draw(|gx, gy, v| {
                        let px = x as i32 + gx as i32 + bbox.min.x as i32 + dx;
                        let py = y_base + gy as i32 + bbox.min.y as i32 + dy;
                        if px >= 0 && py >= 0 {
                            let px = px as u32;
                            let py = py as u32;
                            if let Some(pixel) = get_pixel_mut_checked(canvas, px, py) {
                                let alpha = (v * 255.0) as u8;
                                if alpha > 0 {
                                    let inv_alpha = 255 - alpha;
                                    pixel[0] = ((p.stroke_color[0] as u16 * alpha as u16
                                        + pixel[0] as u16 * inv_alpha as u16)
                                        / 255) as u8;
                                    pixel[1] = ((p.stroke_color[1] as u16 * alpha as u16
                                        + pixel[1] as u16 * inv_alpha as u16)
                                        / 255) as u8;
                                    pixel[2] = ((p.stroke_color[2] as u16 * alpha as u16
                                        + pixel[2] as u16 * inv_alpha as u16)
                                        / 255) as u8;
                                }
                            }
                        }
                    });
                }
            }
        }
    }

    // Fill pass
    for glyph in outlined.iter() {
        let bbox = glyph.px_bounds();
        glyph.draw(|gx, gy, v| {
            let px = x as i32 + gx as i32 + bbox.min.x as i32;
            let py = y_base + gy as i32 + bbox.min.y as i32;
            if px >= 0 && py >= 0 {
                let px = px as u32;
                let py = py as u32;
                if let Some(pixel) = get_pixel_mut_checked(canvas, px, py) {
                    let alpha = (v * 255.0) as u8;
                    if alpha > 0 {
                        let inv_alpha = 255 - alpha;
                        pixel[0] = ((p.color[0] as u16 * alpha as u16
                            + pixel[0] as u16 * inv_alpha as u16)
                            / 255) as u8;
                        pixel[1] = ((p.color[1] as u16 * alpha as u16
                            + pixel[1] as u16 * inv_alpha as u16)
                            / 255) as u8;
                        pixel[2] = ((p.color[2] as u16 * alpha as u16
                            + pixel[2] as u16 * inv_alpha as u16)
                            / 255) as u8;
                    }
                }
            }
        });
    }

    Ok(())
}

struct AddBackgroundParams<'a> {
    font: &'a FontRef<'a>,
    text: &'a str,
    scale: PxScale,
    position: TimestampPosition,
    color: TimestampColor,
    img_width: u32,
    img_height: u32,
    overscan: Option<&'a Overscan>,
    padding_horizontal: u32,
    padding_vertical: u32,
    extra_expand: u32,
}

fn add_text_background(canvas: &mut RgbaImage, p: &AddBackgroundParams) -> Result<()> {
    let (glyphs, width) = layout_line(p.font, p.scale, p.text);
    if glyphs.is_empty() {
        return Ok(());
    }
    let text_width = width.ceil() as u32;
    let text_height = line_height(p.font, p.scale).ceil() as u32;
    let (x, y) = calculate_text_position(&LayoutArea {
        position: p.position,
        text_width,
        text_height,
        area_width: p.img_width,
        area_height: p.img_height,
        area_y_offset: 0,
        overscan: p.overscan,
        padding_horizontal: p.padding_horizontal,
        padding_vertical: p.padding_vertical,
    });

    let bg_color = match p.color {
        TimestampColor::WhiteBackground => Rgba([255, 255, 255, 255]),
        TimestampColor::BlackBackground => Rgba([0, 0, 0, 255]),
        _ => return Ok(()),
    };
    let padding = 4u32 + p.extra_expand;
    for dy in 0..(text_height + padding * 2) {
        for dx in 0..(text_width + padding * 2) {
            let px = x.saturating_sub(padding) + dx;
            let py = y.saturating_sub(padding) + dy;
            if let Some(pixel) = get_pixel_mut_checked(canvas, px, py) {
                *pixel = bg_color;
            }
        }
    }
    Ok(())
}

struct BannerRenderParams<'a> {
    image: DynamicImage,
    font: &'a FontRef<'a>,
    text: &'a str,
    scale: PxScale,
    position: TimestampPosition,
    color: TimestampColor,
    timestamp_config: &'a Timestamp,
    reduced_height: Option<u32>,
    overscan: Option<&'a Overscan>,
}

fn render_banner_timestamp(p: BannerRenderParams) -> Result<DynamicImage> {
    let BannerRenderParams {
        image,
        font,
        text,
        scale,
        position,
        color,
        timestamp_config,
        reduced_height,
        overscan,
    } = p;
    let (img_width, img_height) = image.dimensions();
    let padding_horizontal = timestamp_config.padding_horizontal.unwrap_or(16);
    let padding_vertical = timestamp_config.padding_vertical.unwrap_or(16);

    let padding = 8u32;
    let text_height = scale.y as u32;
    let banner_height = timestamp_config
        .banner_height
        .unwrap_or(text_height + (padding * 2));

    let banner_at_top = matches!(
        position,
        TimestampPosition::TopLeft | TimestampPosition::TopCenter | TimestampPosition::TopRight
    );
    let final_height = reduced_height.unwrap_or(img_height);

    let mut canvas = ImageBuffer::new(img_width, final_height + banner_height);
    for pixel in canvas.pixels_mut() {
        *pixel = Rgba([255, 255, 255, 255]);
    }

    let resized_image = if reduced_height.is_some() {
        image.resize_exact(img_width, final_height, imageops::FilterType::Triangle)
    } else {
        image
    };

    let img_y_offset = if banner_at_top { banner_height } else { 0 };
    image::imageops::overlay(&mut canvas, &resized_image, 0, img_y_offset as i64);

    let banner_y = if banner_at_top { 0 } else { final_height };
    let banner_color = match color {
        TimestampColor::WhiteBackground => Rgba([255, 255, 255, 255]),
        TimestampColor::BlackBackground => Rgba([0, 0, 0, 255]),
        _ => Rgba([255, 255, 255, 255]),
    };
    for y in banner_y..(banner_y + banner_height) {
        for x in 0..img_width {
            if let Some(pixel) = get_pixel_mut_checked(&mut canvas, x, y) {
                *pixel = banner_color;
            }
        }
    }

    let text_color = match color {
        TimestampColor::BlackBackground => Rgba([255, 255, 255, 255]),
        TimestampColor::WhiteBackground => Rgba([0, 0, 0, 255]),
        TimestampColor::TransparentWhiteText => Rgba([255, 255, 255, 255]),
        TimestampColor::TransparentBlackText => Rgba([0, 0, 0, 255]),
        TimestampColor::TransparentAutoText => Rgba([0, 0, 0, 255]),
    };
    let (stroke_enabled, stroke_width, stroke_color) = resolve_stroke(timestamp_config, text_color);
    // For banner, respect left/right overscan always; for vertical, only the side adjacent to the banner.
    let base_left = overscan.map(|o| o.left.max(0)).unwrap_or(0);
    let base_right = overscan.map(|o| o.right.max(0)).unwrap_or(0);
    let base_top = if banner_at_top {
        overscan.map(|o| o.top.max(0)).unwrap_or(0)
    } else {
        0
    };
    let base_bottom = if banner_at_top {
        0
    } else {
        overscan.map(|o| o.bottom.max(0)).unwrap_or(0)
    };
    let banner_osc = Overscan {
        left: base_left,
        right: base_right,
        top: base_top,
        bottom: base_bottom,
    };

    render_text_on_canvas(
        &mut canvas,
        &TextDrawParams {
            font,
            text,
            scale,
            position,
            color: text_color,
            area_y: banner_y,
            area_height: banner_height,
            area_width: img_width,
            // Apply synthesized overscan for correct visible area in banner
            overscan: Some(&banner_osc),
            padding_horizontal,
            padding_vertical,
            stroke_enabled,
            stroke_width,
            stroke_color,
        },
    )?;

    Ok(DynamicImage::ImageRgba8(canvas))
}

struct OverlayRenderParams<'a> {
    image: DynamicImage,
    font: &'a FontRef<'a>,
    text: &'a str,
    scale: PxScale,
    position: TimestampPosition,
    color: TimestampColor,
    overscan: Option<&'a Overscan>,
    timestamp_config: &'a Timestamp,
}

fn render_overlay_timestamp(p: OverlayRenderParams) -> Result<DynamicImage> {
    let OverlayRenderParams {
        image,
        font,
        text,
        scale,
        position,
        color,
        overscan,
        timestamp_config,
    } = p;
    let (img_width, img_height) = image.dimensions();
    let mut canvas = image.to_rgba8();
    let padding_horizontal = timestamp_config.padding_horizontal.unwrap_or(16);
    let padding_vertical = timestamp_config.padding_vertical.unwrap_or(16);

    let text_color = match color {
        TimestampColor::TransparentWhiteText => Rgba([255, 255, 255, 255]),
        TimestampColor::TransparentBlackText => Rgba([0, 0, 0, 255]),
        TimestampColor::BlackBackground => Rgba([255, 255, 255, 255]),
        TimestampColor::WhiteBackground => Rgba([0, 0, 0, 255]),
        TimestampColor::TransparentAutoText => determine_auto_text_color(AutoColorParams {
            canvas: &canvas,
            position,
            scale,
            font,
            text,
            img_width,
            img_height,
            overscan,
            padding_horizontal,
            padding_vertical,
        }),
    };
    let (stroke_enabled, stroke_width, stroke_color) = resolve_stroke(timestamp_config, text_color);

    // Draw background box first (if applicable), so text renders on top of it.
    if matches!(
        color,
        TimestampColor::WhiteBackground | TimestampColor::BlackBackground
    ) {
        add_text_background(
            &mut canvas,
            &AddBackgroundParams {
                font,
                text,
                scale,
                position,
                color,
                img_width,
                img_height,
                overscan,
                padding_horizontal,
                padding_vertical,
                extra_expand: if stroke_enabled { stroke_width } else { 0 },
            },
        )?;
    }

    render_text_on_canvas(
        &mut canvas,
        &TextDrawParams {
            font,
            text,
            scale,
            position,
            color: text_color,
            area_y: 0,
            area_height: img_height,
            area_width: img_width,
            overscan,
            padding_horizontal,
            padding_vertical,
            stroke_enabled,
            stroke_width,
            stroke_color,
        },
    )?;

    Ok(DynamicImage::ImageRgba8(canvas))
}

pub fn render_timestamp(
    image: DynamicImage,
    timestamp_config: &Timestamp,
    reduced_height: Option<u32>,
    date_taken: Option<chrono::DateTime<chrono::Utc>>,
    overscan: Option<&Overscan>,
) -> Result<DynamicImage> {
    if !timestamp_config.enabled {
        return Ok(image);
    }
    let dt = match date_taken {
        Some(d) => d,
        None => return Ok(image),
    };
    // Convert to local time and format. Falls back to %Y-%m-%d
    let local_dt: chrono::DateTime<chrono::Local> = dt.into();
    let fmt = timestamp_config.format.as_deref().unwrap_or("%Y-%m-%d");
    let date_str = local_dt.format(fmt).to_string();
    let font =
        FontRef::try_from_slice(DEFAULT_FONT_DATA).context("failed to parse embedded font")?;

    let font_size = timestamp_config.font_size.unwrap_or(24.0);
    let scale = PxScale::from(font_size);
    let position = timestamp_config.position.unwrap_or_default();
    let color = timestamp_config.color.unwrap_or_default();

    if timestamp_config.full_width_banner {
        render_banner_timestamp(BannerRenderParams {
            image,
            font: &font,
            text: &date_str,
            scale,
            position,
            color,
            timestamp_config,
            reduced_height,
            overscan,
        })
    } else {
        render_overlay_timestamp(OverlayRenderParams {
            image,
            font: &font,
            text: &date_str,
            scale,
            position,
            color,
            overscan,
            timestamp_config,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{TimestampColor, TimestampPosition};

    /// Bounding box (min_x, min_y, max_x, max_y) and count of pixels touched by
    /// white text rendered at the top-left of a black image.
    fn render_footprint(font_size: f32) -> ((u32, u32, u32, u32), usize) {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(240, 80, Rgba([0, 0, 0, 255])));
        let cfg = Timestamp {
            enabled: true,
            position: Some(TimestampPosition::TopLeft),
            font_size: Some(font_size),
            color: Some(TimestampColor::TransparentWhiteText),
            full_width_banner: false,
            banner_height: None,
            padding_horizontal: Some(10),
            padding_vertical: Some(10),
            stroke_enabled: false,
            stroke_width: None,
            stroke_color: None,
            format: Some("%Y-%m-%d".into()),
        };
        let dt = chrono::DateTime::from_timestamp(1_721_390_400, 0).unwrap(); // 2024-07-19 12:00 UTC
        let out = render_timestamp(img, &cfg, None, Some(dt), None)
            .unwrap()
            .to_rgba8();
        let (mut min_x, mut min_y, mut max_x, mut max_y, mut count) = (u32::MAX, u32::MAX, 0, 0, 0);
        for (x, y, p) in out.enumerate_pixels() {
            if p[0] > 0 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                count += 1;
            }
        }
        ((min_x, min_y, max_x, max_y), count)
    }

    #[test]
    fn layout_metrics_match_previous_renderer() {
        let font = FontRef::try_from_slice(DEFAULT_FONT_DATA).unwrap();
        let scale = PxScale::from(24.0);
        let (glyphs, width) = layout_line(&font, scale, "2024-07-19");
        assert_eq!(glyphs.len(), 10);
        // Values produced by the previous rusttype-based layout of the same font.
        assert!((width - 119.8188).abs() < 0.01, "width {width}");
        assert!((line_height(&font, scale) - 24.0).abs() < 0.01);
    }

    #[test]
    fn renders_text_in_expected_area() {
        // Pinned against output from the previous rusttype-based renderer, which
        // was pixel-identical to the current one when migrating to ab_glyph.
        assert_eq!(render_footprint(24.0), ((11, 14, 128, 30), 813));
    }
}
