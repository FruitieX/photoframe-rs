//! Dithering algorithms ported from [dithermark](https://github.com/allen-garvey/dithermark)
//!
//! Diffusion- and ordered-based palette dithering algorithms.
//! Implements only color palette reduction using a luma color dither mode
//! (identity RGB values, distance = luma-weighted squared distance, increment =
//! per-channel addition of propagated RGB error, error amount = raw per-channel
//! delta).

/// Perform in-place dithering & palette reduction on an RGBA buffer.
///
/// pixels: RGBA8 interleaved slice, length must be width * height * 4.
/// palette: slice of RGB triplets. (Alpha always taken from source pixel.)
/// algorithm: optional name (snake/kebab case, case-insensitive). If None or unrecognised, a
///            simple nearest (luma) palette mapping without diffusion is performed.
pub fn dither_image(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    palette: &[[u8; 3]],
    algorithm: Option<&str>,
) {
    if palette.is_empty() || pixels.is_empty() {
        return;
    }
    let algo = algorithm
        .unwrap_or("")
        .to_ascii_lowercase()
        .replace('-', "_");
    if let Some(model) = resolve_model(&algo) {
        diffuse_dither_luma_mode(pixels, width as usize, height as usize, palette, model);
        return;
    }
    if let Some(kind) = resolve_ordered_algorithm(&algo) {
        match kind {
            OrderedKind::Bayer(m) => {
                ordered_bayer_luma(pixels, width as usize, height as usize, palette, m)
            }
            OrderedKind::BlueNoise256 => {
                ordered_blue_luma_256(pixels, width as usize, height as usize, palette)
            }
            OrderedKind::Stark(dim) => {
                ordered_stark_luma(pixels, width as usize, height as usize, palette, dim)
            }
            OrderedKind::Yliluoma1(dim) => {
                ordered_yliluoma1_luma(pixels, width as usize, height as usize, palette, dim)
            }
            OrderedKind::Yliluoma2(dim) => {
                ordered_yliluoma2_luma(pixels, width as usize, height as usize, palette, dim)
            }
        }
        return;
    }
    // Fallback to nearest mapping (no dithering)
    naive_quantize(pixels, palette)
}

fn naive_quantize(pixels: &mut [u8], palette: &[[u8; 3]]) {
    let pal_luma: Vec<f32> = palette.iter().map(|c| luma(c[0], c[1], c[2])).collect();
    for px in pixels.chunks_exact_mut(4) {
        let (r, g, b, a) = (px[0], px[1], px[2], px[3]);
        let lum = luma(r, g, b);
        let mut best = 0usize;
        let mut best_dl = f32::INFINITY;
        let mut best_dist = f32::INFINITY;
        for (i, pal) in palette.iter().enumerate() {
            let dl = (lum - pal_luma[i]).abs();
            if dl < best_dl - 0.01 {
                // prefer clearly closer luma
                best_dl = dl;
                best_dist = color_sq_dist(r, g, b, pal[0], pal[1], pal[2]);
                best = i;
            } else if (dl - best_dl).abs() <= 0.01 {
                // tie: fall back to rgb distance
                let dist = color_sq_dist(r, g, b, pal[0], pal[1], pal[2]);
                if dist < best_dist {
                    best_dist = dist;
                    best = i;
                }
            }
        }
        let pc = palette[best];
        px[0] = pc[0];
        px[1] = pc[1];
        px[2] = pc[2];
        px[3] = a; // preserve alpha
    }
}

#[inline(always)]
fn luma(r: u8, g: u8, b: u8) -> f32 {
    0.299 * (r as f32) + 0.587 * (g as f32) + 0.114 * (b as f32)
}

#[inline(always)]
fn color_sq_dist(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) -> f32 {
    let dr = r1 as f32 - r2 as f32;
    let dg = g1 as f32 - g2 as f32;
    let db = b1 as f32 - b2 as f32;
    dr * dr + dg * dg + db * db
}

#[derive(Clone, Copy)]
struct PropEntry {
    dx: i32,
    dy: usize,
    fraction: f32,
}

#[derive(Clone, Copy)]
struct Model {
    entries: &'static [PropEntry],
    length_offset: usize,
    num_rows: usize,
}

fn resolve_model(name: &str) -> Option<Model> {
    let norm = name.to_ascii_lowercase().replace('-', "_");
    match norm.as_str() {
        "floyd_steinberg" | "fs" => Some(FLOYD_STEINBERG),
        "jarvis_judice_ninke" => Some(JARVIS),
        "stucki" => Some(STUCKI),
        "burkes" => Some(BURKES),
        "sierra_3" => Some(SIERRA3),
        "sierra_2" => Some(SIERRA2),
        // sierra1 is commonly called "sierra lite"
        "sierra_1" | "sierra_lite" | "sierra-lite" => Some(SIERRA1),
        "atkinson" => Some(ATKINSON),
        "reduced_atkinson" => Some(REDUCED_ATKINSON),
        _ => None,
    }
}

// Ordered Bayer matrices (values in range [0, n*n-1]).
// Normalization matches common ordered color dither usage: t = (v + 0.5) / (n*n) - 0.5
// We precompute normalized threshold in [-0.5, 0.5].
#[rustfmt::skip]
const BAYER_2: [[f32; 2]; 2] = {
    let m = [[0u8, 2u8], [3u8, 1u8]];
    const LM1: f32 = 3.0; // length-1 = 4-1
    [
        [((m[0][0] as f32) / LM1 - 0.5), ((m[0][1] as f32) / LM1 - 0.5)],
        [((m[1][0] as f32) / LM1 - 0.5), ((m[1][1] as f32) / LM1 - 0.5)],
    ]
};
#[rustfmt::skip]
const BAYER_4: [[f32; 4]; 4] = {
    let m = [
        [ 0u8,  8u8,  2u8, 10u8],
        [12u8,  4u8, 14u8,  6u8],
        [ 3u8, 11u8,  1u8,  9u8],
        [15u8,  7u8, 13u8,  5u8],
    ];
    const LM1: f32 = 15.0; // length-1 = 16-1
    [
        [((m[0][0] as f32) / LM1 - 0.5), ((m[0][1] as f32) / LM1 - 0.5), ((m[0][2] as f32) / LM1 - 0.5), ((m[0][3] as f32) / LM1 - 0.5)],
        [((m[1][0] as f32) / LM1 - 0.5), ((m[1][1] as f32) / LM1 - 0.5), ((m[1][2] as f32) / LM1 - 0.5), ((m[1][3] as f32) / LM1 - 0.5)],
        [((m[2][0] as f32) / LM1 - 0.5), ((m[2][1] as f32) / LM1 - 0.5), ((m[2][2] as f32) / LM1 - 0.5), ((m[2][3] as f32) / LM1 - 0.5)],
        [((m[3][0] as f32) / LM1 - 0.5), ((m[3][1] as f32) / LM1 - 0.5), ((m[3][2] as f32) / LM1 - 0.5), ((m[3][3] as f32) / LM1 - 0.5)],
    ]
};
#[rustfmt::skip]
const BAYER_8: [[f32; 8]; 8] = {
    let m = [
        [ 0u8, 32u8,  8u8, 40u8,  2u8, 34u8, 10u8, 42u8],
        [48u8, 16u8, 56u8, 24u8, 50u8, 18u8, 58u8, 26u8],
        [12u8, 44u8,  4u8, 36u8, 14u8, 46u8,  6u8, 38u8],
        [60u8, 28u8, 52u8, 20u8, 62u8, 30u8, 54u8, 22u8],
        [ 3u8, 35u8, 11u8, 43u8,  1u8, 33u8,  9u8, 41u8],
        [51u8, 19u8, 59u8, 27u8, 49u8, 17u8, 57u8, 25u8],
        [15u8, 47u8,  7u8, 39u8, 13u8, 45u8,  5u8, 37u8],
        [63u8, 31u8, 55u8, 23u8, 61u8, 29u8, 53u8, 21u8],
    ];
    const LM1: f32 = 63.0; // length-1 = 64-1
    [
        [((m[0][0] as f32) / LM1 - 0.5), ((m[0][1] as f32) / LM1 - 0.5), ((m[0][2] as f32) / LM1 - 0.5), ((m[0][3] as f32) / LM1 - 0.5), ((m[0][4] as f32) / LM1 - 0.5), ((m[0][5] as f32) / LM1 - 0.5), ((m[0][6] as f32) / LM1 - 0.5), ((m[0][7] as f32) / LM1 - 0.5)],
        [((m[1][0] as f32) / LM1 - 0.5), ((m[1][1] as f32) / LM1 - 0.5), ((m[1][2] as f32) / LM1 - 0.5), ((m[1][3] as f32) / LM1 - 0.5), ((m[1][4] as f32) / LM1 - 0.5), ((m[1][5] as f32) / LM1 - 0.5), ((m[1][6] as f32) / LM1 - 0.5), ((m[1][7] as f32) / LM1 - 0.5)],
        [((m[2][0] as f32) / LM1 - 0.5), ((m[2][1] as f32) / LM1 - 0.5), ((m[2][2] as f32) / LM1 - 0.5), ((m[2][3] as f32) / LM1 - 0.5), ((m[2][4] as f32) / LM1 - 0.5), ((m[2][5] as f32) / LM1 - 0.5), ((m[2][6] as f32) / LM1 - 0.5), ((m[2][7] as f32) / LM1 - 0.5)],
        [((m[3][0] as f32) / LM1 - 0.5), ((m[3][1] as f32) / LM1 - 0.5), ((m[3][2] as f32) / LM1 - 0.5), ((m[3][3] as f32) / LM1 - 0.5), ((m[3][4] as f32) / LM1 - 0.5), ((m[3][5] as f32) / LM1 - 0.5), ((m[3][6] as f32) / LM1 - 0.5), ((m[3][7] as f32) / LM1 - 0.5)],
        [((m[4][0] as f32) / LM1 - 0.5), ((m[4][1] as f32) / LM1 - 0.5), ((m[4][2] as f32) / LM1 - 0.5), ((m[4][3] as f32) / LM1 - 0.5), ((m[4][4] as f32) / LM1 - 0.5), ((m[4][5] as f32) / LM1 - 0.5), ((m[4][6] as f32) / LM1 - 0.5), ((m[4][7] as f32) / LM1 - 0.5)],
        [((m[5][0] as f32) / LM1 - 0.5), ((m[5][1] as f32) / LM1 - 0.5), ((m[5][2] as f32) / LM1 - 0.5), ((m[5][3] as f32) / LM1 - 0.5), ((m[5][4] as f32) / LM1 - 0.5), ((m[5][5] as f32) / LM1 - 0.5), ((m[5][6] as f32) / LM1 - 0.5), ((m[5][7] as f32) / LM1 - 0.5)],
        [((m[6][0] as f32) / LM1 - 0.5), ((m[6][1] as f32) / LM1 - 0.5), ((m[6][2] as f32) / LM1 - 0.5), ((m[6][3] as f32) / LM1 - 0.5), ((m[6][4] as f32) / LM1 - 0.5), ((m[6][5] as f32) / LM1 - 0.5), ((m[6][6] as f32) / LM1 - 0.5), ((m[6][7] as f32) / LM1 - 0.5)],
        [((m[7][0] as f32) / LM1 - 0.5), ((m[7][1] as f32) / LM1 - 0.5), ((m[7][2] as f32) / LM1 - 0.5), ((m[7][3] as f32) / LM1 - 0.5), ((m[7][4] as f32) / LM1 - 0.5), ((m[7][5] as f32) / LM1 - 0.5), ((m[7][6] as f32) / LM1 - 0.5), ((m[7][7] as f32) / LM1 - 0.5)],
    ]
};

fn resolve_ordered_algorithm(name: &str) -> Option<OrderedKind> {
    match name {
        "ordered_bayer_2" | "bayer_2" => Some(OrderedKind::Bayer(OrderedMatrix::Bayer2)),
        "ordered_bayer_4" | "bayer_4" => Some(OrderedKind::Bayer(OrderedMatrix::Bayer4)),
        "ordered_bayer_8" | "bayer_8" => Some(OrderedKind::Bayer(OrderedMatrix::Bayer8)),
        "ordered_blue_256" | "blue_256" | "blue_noise_256" => Some(OrderedKind::BlueNoise256),
        "stark" | "stark_8" => Some(OrderedKind::Stark(8)),
        "yliluoma1" | "yliluoma1_8" => Some(OrderedKind::Yliluoma1(8)),
        "yliluoma2" | "yliluoma2_8" => Some(OrderedKind::Yliluoma2(8)),
        _ => None,
    }
}

#[derive(Clone, Copy)]
enum OrderedKind {
    Bayer(OrderedMatrix),
    BlueNoise256,
    Stark(usize),
    Yliluoma1(usize),
    Yliluoma2(usize),
}
#[derive(Clone, Copy)]
enum OrderedMatrix {
    Bayer2,
    Bayer4,
    Bayer8,
}

fn rc(num_colors: usize) -> f32 {
    // Match dithermark worker coefficient: 256 / cbrt(numColors)
    256.0 / (num_colors.max(1) as f32).cbrt()
}

#[inline(always)]
fn to_u8_clamped_f32(x: f32) -> f32 {
    if !x.is_finite() {
        return 0.0;
    }
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 255.0 {
        return 255.0;
    }
    let f = x.floor();
    let frac = x - f;
    let n = f as i32;
    let res = if frac > 0.5 {
        n + 1
    } else if frac < 0.5 || n % 2 == 0 {
        n
    } else {
        n + 1
    };
    res as f32
}

fn ordered_bayer_luma(
    pixels: &mut [u8],
    width: usize,
    height: usize,
    palette: &[[u8; 3]],
    mat: OrderedMatrix,
) {
    const WR: f32 = 0.299;
    const WG: f32 = 0.587;
    const WB: f32 = 0.114;
    let pal_vals: Vec<[f32; 3]> = palette
        .iter()
        .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32])
        .collect();
    let rc = rc(palette.len());
    let (mw, mh) = match mat {
        OrderedMatrix::Bayer2 => (2usize, 2usize),
        OrderedMatrix::Bayer4 => (4, 4),
        OrderedMatrix::Bayer8 => (8, 8),
    };
    for y in 0..height {
        for x in 0..width {
            let i = (y * width + x) * 4;
            let (r0, g0, b0, a) = (
                pixels[i] as f32,
                pixels[i + 1] as f32,
                pixels[i + 2] as f32,
                pixels[i + 3],
            );
            let t = match mat {
                OrderedMatrix::Bayer2 => BAYER_2[y % mh][x % mw],
                OrderedMatrix::Bayer4 => BAYER_4[y % mh][x % mw],
                OrderedMatrix::Bayer8 => BAYER_8[y % mh][x % mw],
            };
            // Emulate Uint8ClampedArray assignment rounding/clamping
            let pr = to_u8_clamped_f32(r0 + t * rc);
            let pg = to_u8_clamped_f32(g0 + t * rc);
            let pb = to_u8_clamped_f32(b0 + t * rc);
            let mut best = 0usize;
            let mut best_dist = f32::INFINITY;
            for (idx, pv) in pal_vals.iter().enumerate() {
                let dr = pr - pv[0];
                let dg = pg - pv[1];
                let db = pb - pv[2];
                let dist = dr * dr * WR + dg * dg * WG + db * db * WB;
                if dist < best_dist {
                    best_dist = dist;
                    best = idx;
                }
            }
            let chosen = pal_vals[best];
            pixels[i] = chosen[0] as u8;
            pixels[i + 1] = chosen[1] as u8;
            pixels[i + 2] = chosen[2] as u8;
            pixels[i + 3] = a;
        }
    }
}

// ----- Ordered Blue-noise (256x256 mask) -----
use once_cell::sync::OnceCell;

static BLUE_MASK_BYTES: &[u8] = include_bytes!("../assets/256x256_blue.png");
static BLUE_MASK: OnceCell<(usize, usize, Box<[u8]>)> = OnceCell::new();

fn load_blue_mask() -> &'static (usize, usize, Box<[u8]>) {
    BLUE_MASK.get_or_init(|| {
        let img = image::load_from_memory(BLUE_MASK_BYTES)
            .expect("embedded blue noise mask png should decode");
        let gray = img.to_luma8();
        let (w, h) = gray.dimensions();
        (w as usize, h as usize, gray.into_raw().into_boxed_slice())
    })
}

fn ordered_blue_luma_256(pixels: &mut [u8], width: usize, height: usize, palette: &[[u8; 3]]) {
    const WR: f32 = 0.299;
    const WG: f32 = 0.587;
    const WB: f32 = 0.114;
    let pal_vals: Vec<[f32; 3]> = palette
        .iter()
        .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32])
        .collect();
    let rc = rc(palette.len());
    let (mw, mh, mask) = {
        let (w, h, data) = load_blue_mask();
        (*w, *h, data)
    };
    for y in 0..height {
        let my = y % mh;
        for x in 0..width {
            let mx = x % mw;
            let i = (y * width + x) * 4;
            let (r0, g0, b0, a) = (
                pixels[i] as f32,
                pixels[i + 1] as f32,
                pixels[i + 2] as f32,
                pixels[i + 3],
            );
            let mval = mask[my * mw + mx] as f32; // 0..255
            let t = mval / 255.0 - 0.5; // [-0.5, 0.5]
            let pr = to_u8_clamped_f32(r0 + t * rc);
            let pg = to_u8_clamped_f32(g0 + t * rc);
            let pb = to_u8_clamped_f32(b0 + t * rc);
            let mut best = 0usize;
            let mut best_dist = f32::INFINITY;
            for (idx, pv) in pal_vals.iter().enumerate() {
                let dr = pr - pv[0];
                let dg = pg - pv[1];
                let db = pb - pv[2];
                let dist = dr * dr * WR + dg * dg * WG + db * db * WB;
                if dist < best_dist {
                    best_dist = dist;
                    best = idx;
                }
            }
            let chosen = pal_vals[best];
            pixels[i] = chosen[0] as u8;
            pixels[i + 1] = chosen[1] as u8;
            pixels[i + 2] = chosen[2] as u8;
            pixels[i + 3] = a;
        }
    }
}

// Integer Bayer matrices for Stark/Yliluoma paths
#[rustfmt::skip]
const BAYER_2_I: [[u8; 2]; 2] = [[0, 2],[3, 1]];
#[rustfmt::skip]
const BAYER_4_I: [[u8; 4]; 4] = [
    [ 0,  8,  2, 10],
    [12,  4, 14,  6],
    [ 3, 11,  1,  9],
    [15,  7, 13,  5],
];
#[rustfmt::skip]
const BAYER_8_I: [[u8; 8]; 8] = [
    [ 0, 32,  8, 40,  2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44,  4, 36, 14, 46,  6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [ 3, 35, 11, 43,  1, 33,  9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47,  7, 39, 13, 45,  5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

fn bayer_index(dim: usize, x: usize, y: usize) -> u8 {
    match dim {
        2 => BAYER_2_I[y % 2][x % 2],
        4 => BAYER_4_I[y % 4][x % 4],
        _ => BAYER_8_I[y % 8][x % 8],
    }
}

fn ordered_stark_luma(
    pixels: &mut [u8],
    width: usize,
    height: usize,
    palette: &[[u8; 3]],
    dim: usize,
) {
    const WR: f32 = 0.299;
    const WG: f32 = 0.587;
    const WB: f32 = 0.114;
    let pal_vals: Vec<[f32; 3]> = palette
        .iter()
        .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32])
        .collect();
    // Stark uses WebGL coefficient in reference: 1.0 / cbrt(num_colors)
    let rc = 1.0 / (palette.len().max(1) as f32).cbrt();
    let length = (dim * dim) as f32;
    let fraction = 1.0 / (length - 1.0);
    // Precompute Stark matrix as flat vec
    let mut stark: Vec<f32> = vec![0.0; dim * dim];
    for y in 0..dim {
        for x in 0..dim {
            let base = bayer_index(dim, x, y) as f32;
            stark[y * dim + x] = 1.0 - base * fraction * rc;
        }
    }
    for y in 0..height {
        for x in 0..width {
            let i = (y * width + x) * 4;
            let (r0, g0, b0, a) = (
                pixels[i] as f32,
                pixels[i + 1] as f32,
                pixels[i + 2] as f32,
                pixels[i + 3],
            );
            let bayer_value = stark[(y % dim) * dim + (x % dim)];
            let pr = r0;
            let pg = g0;
            let pb = b0;
            // nearest by LUMA-weighted distance
            let mut shortest = f32::INFINITY;
            let mut shortest_idx = 0usize;
            for (idx, pv) in pal_vals.iter().enumerate() {
                let dr = pr - pv[0];
                let dg = pg - pv[1];
                let db = pb - pv[2];
                let dist = dr * dr * WR + dg * dg * WG + db * db * WB;
                if dist < shortest {
                    shortest = dist;
                    shortest_idx = idx;
                }
            }
            let mut pixel_match_idx = shortest_idx;
            if bayer_value < 1.0 {
                // always true in practice per reference impl
                let mut greatest_allowed = -1.0f32;
                let mut greatest_idx = shortest_idx;
                for (idx, pv) in pal_vals.iter().enumerate() {
                    let dr = pr - pv[0];
                    let dg = pg - pv[1];
                    let db = pb - pv[2];
                    let dist = dr * dr * WR + dg * dg * WG + db * db * WB;
                    if dist > greatest_allowed && (dist / shortest) * bayer_value < 1.0 {
                        greatest_allowed = dist;
                        greatest_idx = idx;
                    }
                }
                pixel_match_idx = greatest_idx;
            }
            let chosen = pal_vals[pixel_match_idx];
            pixels[i] = chosen[0] as u8;
            pixels[i + 1] = chosen[1] as u8;
            pixels[i + 2] = chosen[2] as u8;
            pixels[i + 3] = a;
        }
    }
}

fn ordered_yliluoma1_luma(
    pixels: &mut [u8],
    width: usize,
    height: usize,
    palette: &[[u8; 3]],
    dim: usize,
) {
    const WR: f32 = 0.299;
    const WG: f32 = 0.587;
    const WB: f32 = 0.114;
    let color_values: Vec<[f32; 3]> = palette
        .iter()
        .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32])
        .collect();
    let matrix_len = (dim * dim) as f32;
    let mut mix_pixel = [0f32; 3];
    for y in 0..height {
        for x in 0..width {
            let i = (y * width + x) * 4;
            let (r0, g0, b0, a) = (
                pixels[i] as f32,
                pixels[i + 1] as f32,
                pixels[i + 2] as f32,
                pixels[i + 3],
            );
            let pixel_value = [r0, g0, b0];
            let bayer_value = bayer_index(dim, x, y) as f32;

            let mut color_index1 = 0usize;
            let mut color_index2 = 0usize;
            let mut lowest_ratio = 0f32;
            let mut least_penalty = f32::INFINITY;
            for i1 in 0..palette.len() {
                for i2 in i1..palette.len() {
                    for ratio in 0..(matrix_len as usize) {
                        if i1 == i2 && ratio != 0 {
                            break;
                        }
                        let c1 = color_values[i1];
                        let c2 = color_values[i2];
                        mix_pixel[0] = (c1[0] + (ratio as f32 * (c2[0] - c1[0]) / matrix_len))
                            .floor()
                            .clamp(0.0, 255.0);
                        mix_pixel[1] = (c1[1] + (ratio as f32 * (c2[1] - c1[1]) / matrix_len))
                            .floor()
                            .clamp(0.0, 255.0);
                        mix_pixel[2] = (c1[2] + (ratio as f32 * (c2[2] - c1[2]) / matrix_len))
                            .floor()
                            .clamp(0.0, 255.0);
                        let dr = pixel_value[0] - mix_pixel[0];
                        let dg = pixel_value[1] - mix_pixel[1];
                        let db = pixel_value[2] - mix_pixel[2];
                        let mix_dist = dr * dr * WR + dg * dg * WG + db * db * WB;
                        let d1r = c1[0] - c2[0];
                        let d1g = c1[1] - c2[1];
                        let d1b = c1[2] - c2[2];
                        let color_pair_dist = d1r * d1r * WR + d1g * d1g * WG + d1b * d1b * WB;
                        let ratio_fraction = (ratio as f32) / matrix_len;
                        let penalty =
                            mix_dist + color_pair_dist * 0.1 * ((ratio_fraction - 0.5).abs() + 0.5);
                        if penalty < least_penalty {
                            least_penalty = penalty;
                            color_index1 = i1;
                            color_index2 = i2;
                            lowest_ratio = ratio as f32;
                        }
                    }
                }
            }
            let pick = if bayer_value < lowest_ratio {
                color_index2
            } else {
                color_index1
            };
            let chosen = color_values[pick];
            pixels[i] = chosen[0] as u8;
            pixels[i + 1] = chosen[1] as u8;
            pixels[i + 2] = chosen[2] as u8;
            pixels[i + 3] = a;
        }
    }
}

fn ordered_yliluoma2_luma(
    pixels: &mut [u8],
    width: usize,
    height: usize,
    palette: &[[u8; 3]],
    dim: usize,
) {
    let colors_len = palette.len();
    if colors_len == 0 {
        return;
    }
    // Precompute palette lumas scaled as in reference (no divide by 1000)
    let mut palette_values: Vec<u32> = vec![0; colors_len];
    for (i, c) in palette.iter().enumerate() {
        palette_values[i] = (c[0] as u32) * 299 + (c[1] as u32) * 587 + (c[2] as u32) * 114;
    }
    let matrix_len = dim * dim;
    let mut plan_buffer: Vec<usize> = vec![0; colors_len];

    for y in 0..height {
        for x in 0..width {
            let i = (y * width + x) * 4;
            let (r0, g0, b0, a) = (
                pixels[i] as f32,
                pixels[i + 1] as f32,
                pixels[i + 2] as f32,
                pixels[i + 3],
            );
            let pixel_value = [r0, g0, b0];
            let bayer_value = bayer_index(dim, x, y) as usize;
            let plan_index = (bayer_value * colors_len) / matrix_len;

            // Devise mixing plan
            let mut proportion_total = 0usize;
            let mut so_far = [0u32; 3];
            while proportion_total < colors_len {
                let mut chosen_amount = 1usize;
                let mut chosen = 0usize;
                let max_test_count = proportion_total.max(1);
                let mut least_penalty = f32::INFINITY;
                for (idx, color) in palette.iter().copied().enumerate() {
                    let mut sum = so_far;
                    let mut add = [color[0] as u32, color[1] as u32, color[2] as u32];
                    let mut p = 1usize;
                    while p <= max_test_count {
                        for c in 0..3 {
                            sum[c] += add[c];
                            add[c] += add[c];
                        }
                        let t = (proportion_total + p) as f32;
                        // Emulate integer typed array assignment (floor)
                        let test = [
                            ((sum[0] as f32 / t).floor()).clamp(0.0, 255.0),
                            ((sum[1] as f32 / t).floor()).clamp(0.0, 255.0),
                            ((sum[2] as f32 / t).floor()).clamp(0.0, 255.0),
                        ];
                        let dr = pixel_value[0] - test[0];
                        let dg = pixel_value[1] - test[1];
                        let db = pixel_value[2] - test[2];
                        let penalty = dr * dr * 0.299 + dg * dg * 0.587 + db * db * 0.114;
                        if penalty < least_penalty {
                            least_penalty = penalty;
                            chosen = idx;
                            chosen_amount = p;
                        }
                        p *= 2;
                    }
                }
                for _ in 0..chosen_amount {
                    if proportion_total >= colors_len {
                        break;
                    }
                    plan_buffer[proportion_total] = chosen;
                    proportion_total += 1;
                }
                let c = palette[chosen];
                so_far[0] += c[0] as u32 * chosen_amount as u32;
                so_far[1] += c[1] as u32 * chosen_amount as u32;
                so_far[2] += c[2] as u32 * chosen_amount as u32;
            }
            // Sort by palette luma ascending
            plan_buffer.sort_by_key(|&idx| palette_values[idx]);
            let chosen = palette[plan_buffer[plan_index]];
            pixels[i] = chosen[0];
            pixels[i + 1] = chosen[1];
            pixels[i + 2] = chosen[2];
            pixels[i + 3] = a;
        }
    }
}

// Static model definitions.
macro_rules! model {($name:ident, $len_off:expr, $rows:expr, [ $( ($dx:expr,$dy:expr,$frac:expr) ),* $(,)? ]) => {
    const $name: Model = Model { entries: &[ $( PropEntry { dx: $dx, dy: $dy, fraction: $frac } ),* ], length_offset: $len_off, num_rows: $rows };};}

model!(
    FLOYD_STEINBERG,
    1,
    2,
    [
        (1, 0, 7.0 / 16.0),
        (1, 1, 1.0 / 16.0),
        (0, 1, 5.0 / 16.0),
        (-1, 1, 3.0 / 16.0)
    ]
);
model!(
    JARVIS,
    2,
    3,
    [
        (1, 0, 7.0 / 48.0),
        (2, 0, 5.0 / 48.0),
        (-2, 1, 3.0 / 48.0),
        (-1, 1, 5.0 / 48.0),
        (0, 1, 7.0 / 48.0),
        (1, 1, 5.0 / 48.0),
        (2, 1, 3.0 / 48.0),
        (-2, 2, 1.0 / 48.0),
        (-1, 2, 3.0 / 48.0),
        (0, 2, 5.0 / 48.0),
        (1, 2, 3.0 / 48.0),
        (2, 2, 1.0 / 48.0)
    ]
);
model!(
    STUCKI,
    2,
    3,
    [
        (1, 0, 8.0 / 42.0),
        (2, 0, 4.0 / 42.0),
        (-2, 1, 2.0 / 42.0),
        (-1, 1, 4.0 / 42.0),
        (0, 1, 8.0 / 42.0),
        (1, 1, 4.0 / 42.0),
        (2, 1, 2.0 / 42.0),
        (-2, 2, 1.0 / 42.0),
        (-1, 2, 2.0 / 42.0),
        (0, 2, 4.0 / 42.0),
        (1, 2, 2.0 / 42.0),
        (2, 2, 1.0 / 42.0)
    ]
);
model!(
    BURKES,
    2,
    2,
    [
        (1, 0, 8.0 / 32.0),
        (2, 0, 4.0 / 32.0),
        (-2, 1, 2.0 / 32.0),
        (-1, 1, 4.0 / 32.0),
        (0, 1, 8.0 / 32.0),
        (1, 1, 4.0 / 32.0),
        (2, 1, 2.0 / 32.0)
    ]
);
model!(
    SIERRA3,
    2,
    3,
    [
        (1, 0, 5.0 / 32.0),
        (2, 0, 3.0 / 32.0),
        (-2, 1, 2.0 / 32.0),
        (-1, 1, 4.0 / 32.0),
        (0, 1, 5.0 / 32.0),
        (1, 1, 4.0 / 32.0),
        (2, 1, 2.0 / 32.0),
        (-1, 2, 2.0 / 32.0),
        (0, 2, 3.0 / 32.0),
        (1, 2, 2.0 / 32.0)
    ]
);
model!(
    SIERRA2,
    2,
    2,
    [
        (1, 0, 4.0 / 16.0),
        (2, 0, 3.0 / 16.0),
        (-2, 1, 1.0 / 16.0),
        (-1, 1, 2.0 / 16.0),
        (0, 1, 3.0 / 16.0),
        (1, 1, 2.0 / 16.0),
        (2, 1, 1.0 / 16.0)
    ]
);
model!(
    SIERRA1,
    1,
    2,
    [(1, 0, 2.0 / 4.0), (-1, 1, 1.0 / 4.0), (0, 1, 1.0 / 4.0)]
);
model!(
    ATKINSON,
    2,
    3,
    [
        (1, 0, 1.0 / 8.0),
        (2, 0, 1.0 / 8.0),
        (-1, 1, 1.0 / 8.0),
        (0, 1, 1.0 / 8.0),
        (1, 1, 1.0 / 8.0),
        (0, 2, 1.0 / 8.0)
    ]
);
model!(
    REDUCED_ATKINSON,
    2,
    2,
    [
        (1, 0, 2.0 / 16.0),
        (2, 0, 1.0 / 16.0),
        (0, 1, 2.0 / 16.0),
        (1, 1, 1.0 / 16.0)
    ]
);

fn diffuse_dither_luma_mode(
    pixels: &mut [u8],
    width: usize,
    height: usize,
    palette: &[[u8; 3]],
    model: Model,
) {
    // Precompute palette value vectors used for distance comparisons (identity RGB) & weights for distance.
    let pal_vals: Vec<[f32; 3]> = palette
        .iter()
        .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32])
        .collect();
    // Luma distance function weights applied to squared channel deltas.
    const WR: f32 = 0.299;
    const WG: f32 = 0.587;
    const WB: f32 = 0.114;
    // Error propagation matrix: per-channel (dimensions=3) ring buffer
    let row_stride = (width + model.length_offset * 2) * 3; // packed RGB
    let mut rows: Vec<Vec<f32>> = (0..model.num_rows).map(|_| vec![0.0; row_stride]).collect();

    for y in 0..height {
        // base offset inside the row for x=0 (skip left padding) * 3 channels
        let mut base = model.length_offset * 3;
        for x in 0..width {
            let i = (y * width + x) * 4;
            let (r0, g0, b0, a) = (
                pixels[i] as f32,
                pixels[i + 1] as f32,
                pixels[i + 2] as f32,
                pixels[i + 3],
            );
            let er = rows[0][base];
            let eg = rows[0][base + 1];
            let eb = rows[0][base + 2];
            let pr = (r0 + er).clamp(0.0, 255.0);
            let pg = (g0 + eg).clamp(0.0, 255.0);
            let pb = (b0 + eb).clamp(0.0, 255.0);

            // Find closest palette index using luma-weighted squared RGB distance.
            let mut best = 0usize;
            let mut best_dist = f32::INFINITY;
            for (idx, pv) in pal_vals.iter().enumerate() {
                let dr = pr - pv[0];
                let dg = pg - pv[1];
                let db = pb - pv[2];
                let dist = dr * dr * WR + dg * dg * WG + db * db * WB;
                if dist < best_dist {
                    best_dist = dist;
                    best = idx;
                }
            }
            let chosen = pal_vals[best];
            pixels[i] = chosen[0] as u8;
            pixels[i + 1] = chosen[1] as u8;
            pixels[i + 2] = chosen[2] as u8;
            pixels[i + 3] = a;

            // Error (expected - actual).
            let er_out = pr - chosen[0];
            let eg_out = pg - chosen[1];
            let eb_out = pb - chosen[2];
            if er_out != 0.0 || eg_out != 0.0 || eb_out != 0.0 {
                for entry in model.entries.iter() {
                    let nx = (base as isize) + (entry.dx as isize) * 3;
                    if nx < 0 || nx as usize >= row_stride {
                        continue;
                    }
                    let row = entry.dy;
                    if row < model.num_rows {
                        let dst = &mut rows[row][nx as usize..nx as usize + 3];
                        dst[0] += er_out * entry.fraction;
                        dst[1] += eg_out * entry.fraction;
                        dst[2] += eb_out * entry.fraction;
                    }
                }
            }
            base += 3;
        }
        // rotate & zero first row
        let mut first = rows.remove(0);
        first.fill(0.0);
        rows.push(first);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_dither_runs() {
        let mut img = vec![0u8; 16 * 16 * 4];
        // gradient pattern
        for y in 0..16 {
            for x in 0..16 {
                let i = (y * 16 + x) * 4;
                img[i] = (x * 16) as u8;
                img[i + 1] = (y * 16) as u8;
                img[i + 2] = (((x + y) / 2) * 16) as u8;
                img[i + 3] = 255;
            }
        }
        let palette = [
            [0, 0, 0],
            [255, 255, 255],
            [255, 0, 0],
            [0, 255, 0],
            [0, 0, 255],
        ];
        dither_image(&mut img, 16, 16, &palette, Some("floyd_steinberg"));
        // all pixels should be from palette
        for px in img.chunks_exact(4) {
            assert!(
                palette
                    .iter()
                    .any(|c| c[0] == px[0] && c[1] == px[1] && c[2] == px[2])
            );
        }
    }

    #[test]
    fn ordered_bayer_runs() {
        let mut img = vec![0u8; 8 * 8 * 4];
        for y in 0..8 {
            for x in 0..8 {
                let i = (y * 8 + x) * 4;
                img[i] = (x * 32) as u8;
                img[i + 1] = (y * 32) as u8;
                img[i + 2] = 128;
                img[i + 3] = 255;
            }
        }
        let palette = [[0, 0, 0], [255, 255, 255], [255, 0, 0], [0, 255, 0]];
        dither_image(&mut img, 8, 8, &palette, Some("ordered_bayer_8"));
        for px in img.chunks_exact(4) {
            assert!(
                palette
                    .iter()
                    .any(|c| c[0] == px[0] && c[1] == px[1] && c[2] == px[2])
            );
        }
    }

    #[test]
    fn ordered_stark_runs() {
        let mut img = vec![0u8; 8 * 8 * 4];
        for y in 0..8 {
            for x in 0..8 {
                let i = (y * 8 + x) * 4;
                img[i] = (x * 32) as u8;
                img[i + 1] = (y * 32) as u8;
                img[i + 2] = 180;
                img[i + 3] = 255;
            }
        }
        let palette = [[0, 0, 0], [255, 255, 255], [255, 0, 0], [0, 255, 0]];
        dither_image(&mut img, 8, 8, &palette, Some("stark_8"));
        for px in img.chunks_exact(4) {
            assert!(
                palette
                    .iter()
                    .any(|c| c[0] == px[0] && c[1] == px[1] && c[2] == px[2])
            );
        }
    }

    #[test]
    fn ordered_yliluoma_runs() {
        let mut img = vec![0u8; 8 * 8 * 4];
        for y in 0..8 {
            for x in 0..8 {
                let i = (y * 8 + x) * 4;
                img[i] = (x * 32) as u8;
                img[i + 1] = (y * 32) as u8;
                img[i + 2] = 100;
                img[i + 3] = 255;
            }
        }
        let palette = [[0, 0, 0], [255, 255, 255], [255, 0, 0], [0, 255, 0]];
        dither_image(&mut img, 8, 8, &palette, Some("yliluoma1_8"));
        for px in img.chunks_exact(4) {
            assert!(
                palette
                    .iter()
                    .any(|c| c[0] == px[0] && c[1] == px[1] && c[2] == px[2])
            );
        }
        dither_image(&mut img, 8, 8, &palette, Some("yliluoma2_8"));
        for px in img.chunks_exact(4) {
            assert!(
                palette
                    .iter()
                    .any(|c| c[0] == px[0] && c[1] == px[1] && c[2] == px[2])
            );
        }
    }

    #[test]
    fn ordered_blue_mask_runs() {
        let mut img = vec![0u8; 32 * 32 * 4];
        for y in 0..32 {
            for x in 0..32 {
                let i = (y * 32 + x) * 4;
                img[i] = (x * 8) as u8;
                img[i + 1] = (y * 8) as u8;
                img[i + 2] = 120;
                img[i + 3] = 255;
            }
        }
        let palette = [[0, 0, 0], [255, 255, 255], [255, 0, 0], [0, 255, 0]];
        dither_image(&mut img, 32, 32, &palette, Some("ordered_blue_256"));
        for px in img.chunks_exact(4) {
            assert!(
                palette
                    .iter()
                    .any(|c| c[0] == px[0] && c[1] == px[1] && c[2] == px[2])
            );
        }
    }
}
