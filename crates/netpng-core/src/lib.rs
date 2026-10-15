use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};

use anyhow::{bail, ensure, Context};

/// An image decoded into 8-bit RGBA, regardless of the source PNG's
/// original color type or bit depth. This is the common currency every
/// netpng tool passes between decode and encode.
#[derive(Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// Reads the RGBA value of a single pixel. Panics if `(x, y)` is out of bounds.
pub fn pixel_at(img: &Image, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * img.width + x) * 4) as usize;
    [img.pixels[i], img.pixels[i + 1], img.pixels[i + 2], img.pixels[i + 3]]
}

/// Crops `img` to the rectangle `(x, y, width, height)`, shared by every
/// tool that needs to cut out a sub-rectangle.
pub fn crop(img: &Image, x: u32, y: u32, width: u32, height: u32) -> anyhow::Result<Image> {
    ensure!(
        x.checked_add(width).is_some_and(|r| r <= img.width)
            && y.checked_add(height).is_some_and(|r| r <= img.height),
        "crop rectangle ({x}, {y}, {width}x{height}) is out of bounds for a {}x{} image",
        img.width,
        img.height
    );
    let mut pixels = vec![0u8; (width as usize) * (height as usize) * 4];
    for row in 0..height {
        let src_start = (((y + row) * img.width + x) * 4) as usize;
        let src_end = src_start + (width * 4) as usize;
        let dst_start = (row * width * 4) as usize;
        let dst_end = dst_start + (width * 4) as usize;
        pixels[dst_start..dst_end].copy_from_slice(&img.pixels[src_start..src_end]);
    }
    Ok(Image { width, height, pixels })
}

/// Alpha-composites `overlay` onto `base` using the standard Porter-Duff
/// "over" operator, placing the overlay's top-left corner at `(x, y)` on
/// the base canvas. The result has `base`'s dimensions; any part of
/// `overlay` that falls outside those bounds is clipped silently.
pub fn composite_over(base: &Image, overlay: &Image, x: i32, y: i32) -> Image {
    let mut pixels = base.pixels.clone();
    for oy in 0..overlay.height {
        let by = y + oy as i32;
        if by < 0 || by >= base.height as i32 {
            continue;
        }
        for ox in 0..overlay.width {
            let bx = x + ox as i32;
            if bx < 0 || bx >= base.width as i32 {
                continue;
            }
            let src = pixel_at(overlay, ox, oy);
            let idx = ((by as u32 * base.width + bx as u32) * 4) as usize;
            let dst = [pixels[idx], pixels[idx + 1], pixels[idx + 2], pixels[idx + 3]];
            let blended = blend_over(src, dst);
            pixels[idx..idx + 4].copy_from_slice(&blended);
        }
    }
    Image {
        width: base.width,
        height: base.height,
        pixels,
    }
}

fn blend_over(src: [u8; 4], dst: [u8; 4]) -> [u8; 4] {
    let sa = src[3] as f32 / 255.0;
    let da = dst[3] as f32 / 255.0;
    let oa = sa + da * (1.0 - sa);
    if oa <= 0.0 {
        return [0, 0, 0, 0];
    }
    let mut out = [0u8; 4];
    for c in 0..3 {
        let sc = src[c] as f32 / 255.0;
        let dc = dst[c] as f32 / 255.0;
        let oc = (sc * sa + dc * da * (1.0 - sa)) / oa;
        out[c] = (oc * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    out[3] = (oa * 255.0).round().clamp(0.0, 255.0) as u8;
    out
}

/// Inverts the RGB channels of every pixel (255 - v); alpha is untouched.
pub fn invert(img: &Image) -> Image {
    let mut pixels = img.pixels.clone();
    for px in pixels.chunks_exact_mut(4) {
        px[0] = 255 - px[0];
        px[1] = 255 - px[1];
        px[2] = 255 - px[2];
    }
    Image {
        width: img.width,
        height: img.height,
        pixels,
    }
}

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum FlipMode {
    Horizontal,
    Vertical,
    Rotate90,
    Rotate180,
    Rotate270,
}

/// Mirrors or rotates `img` by a multiple of 90 degrees (like pamflip).
pub fn flip(img: &Image, mode: FlipMode) -> Image {
    let (in_w, in_h) = (img.width, img.height);
    let (out_w, out_h) = match mode {
        FlipMode::Rotate90 | FlipMode::Rotate270 => (in_h, in_w),
        _ => (in_w, in_h),
    };
    let mut pixels = vec![0u8; (out_w as usize) * (out_h as usize) * 4];
    for oy in 0..out_h {
        for ox in 0..out_w {
            let (sx, sy) = match mode {
                FlipMode::Horizontal => (in_w - 1 - ox, oy),
                FlipMode::Vertical => (ox, in_h - 1 - oy),
                FlipMode::Rotate90 => (oy, in_h - 1 - ox),
                FlipMode::Rotate180 => (in_w - 1 - ox, in_h - 1 - oy),
                FlipMode::Rotate270 => (in_w - 1 - oy, ox),
            };
            let src = pixel_at(img, sx, sy);
            let idx = ((oy * out_w + ox) * 4) as usize;
            pixels[idx..idx + 4].copy_from_slice(&src);
        }
    }
    Image {
        width: out_w,
        height: out_h,
        pixels,
    }
}

fn sample_bilinear(img: &Image, x: f64, y: f64) -> [u8; 4] {
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(img.width - 1);
    let y1 = (y0 + 1).min(img.height - 1);
    let fx = x - x0 as f64;
    let fy = y - y0 as f64;

    let p00 = pixel_at(img, x0, y0);
    let p10 = pixel_at(img, x1, y0);
    let p01 = pixel_at(img, x0, y1);
    let p11 = pixel_at(img, x1, y1);

    let mut out = [0u8; 4];
    for c in 0..4 {
        let top = p00[c] as f64 * (1.0 - fx) + p10[c] as f64 * fx;
        let bottom = p01[c] as f64 * (1.0 - fx) + p11[c] as f64 * fx;
        out[c] = (top * (1.0 - fy) + bottom * fy).round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// Resizes `img` to `out_width` x `out_height` using bilinear interpolation.
pub fn scale(img: &Image, out_width: u32, out_height: u32) -> Image {
    let mut pixels = vec![0u8; (out_width as usize) * (out_height as usize) * 4];
    let x_ratio = img.width as f64 / out_width as f64;
    let y_ratio = img.height as f64 / out_height as f64;
    for oy in 0..out_height {
        let src_y = ((oy as f64 + 0.5) * y_ratio - 0.5).clamp(0.0, (img.height - 1) as f64);
        for ox in 0..out_width {
            let src_x = ((ox as f64 + 0.5) * x_ratio - 0.5).clamp(0.0, (img.width - 1) as f64);
            let color = sample_bilinear(img, src_x, src_y);
            let idx = ((oy * out_width + ox) * 4) as usize;
            pixels[idx..idx + 4].copy_from_slice(&color);
        }
    }
    Image {
        width: out_width,
        height: out_height,
        pixels,
    }
}

/// Rotates `img` by `angle_degrees` clockwise about its center,
/// expanding the canvas to fit the whole rotated image and filling the
/// newly exposed corners with `background`. Uses bilinear sampling.
pub fn rotate(img: &Image, angle_degrees: f64, background: [u8; 4]) -> Image {
    let angle = angle_degrees.to_radians();
    let (cos_a, sin_a) = (angle.cos(), angle.sin());
    let (in_w, in_h) = (img.width as f64, img.height as f64);

    let out_w = (in_w * cos_a.abs() + in_h * sin_a.abs()).ceil().max(1.0) as u32;
    let out_h = (in_w * sin_a.abs() + in_h * cos_a.abs()).ceil().max(1.0) as u32;

    let center_in = ((in_w - 1.0) / 2.0, (in_h - 1.0) / 2.0);
    let center_out = ((out_w as f64 - 1.0) / 2.0, (out_h as f64 - 1.0) / 2.0);

    let mut pixels = vec![0u8; (out_w as usize) * (out_h as usize) * 4];
    for oy in 0..out_h {
        let dy = oy as f64 - center_out.1;
        for ox in 0..out_w {
            let dx = ox as f64 - center_out.0;
            // Inverse-rotate the output offset back into input space.
            let src_dx = dx * cos_a + dy * sin_a;
            let src_dy = -dx * sin_a + dy * cos_a;
            let src_x = src_dx + center_in.0;
            let src_y = src_dy + center_in.1;

            let idx = ((oy * out_w + ox) * 4) as usize;
            let color = if src_x >= 0.0 && src_x <= in_w - 1.0 && src_y >= 0.0 && src_y <= in_h - 1.0 {
                sample_bilinear(img, src_x, src_y)
            } else {
                background
            };
            pixels[idx..idx + 4].copy_from_slice(&color);
        }
    }
    Image {
        width: out_w,
        height: out_h,
        pixels,
    }
}

/// Linearly stretches each of R, G, B independently so that `bpercent`% of
/// the darkest pixel values map to 0 and `wpercent`% of the brightest map
/// to 255 (like pnmnorm's default, per-channel mode). Alpha is untouched.
pub fn normalize(img: &Image, bpercent: f64, wpercent: f64) -> Image {
    let total = (img.width as u64) * (img.height as u64);
    let mut histograms = [[0u64; 256]; 3];
    for px in img.pixels.chunks_exact(4) {
        for c in 0..3 {
            histograms[c][px[c] as usize] += 1;
        }
    }

    let black_count = ((bpercent / 100.0) * total as f64).round() as u64;
    let white_count = ((wpercent / 100.0) * total as f64).round() as u64;

    let mut ranges = [(0u8, 255u8); 3];
    for (c, hist) in histograms.iter().enumerate() {
        let mut cum = 0u64;
        let mut black_point = 0u8;
        for v in 0..=255u8 {
            cum += hist[v as usize];
            if cum > black_count {
                black_point = v;
                break;
            }
        }
        let mut cum = 0u64;
        let mut white_point = 255u8;
        for v in (0..=255u8).rev() {
            cum += hist[v as usize];
            if cum > white_count {
                white_point = v;
                break;
            }
        }
        ranges[c] = if white_point > black_point {
            (black_point, white_point)
        } else {
            (0, 255)
        };
    }

    let mut pixels = img.pixels.clone();
    for px in pixels.chunks_exact_mut(4) {
        for (c, &(bp, wp)) in ranges.iter().enumerate() {
            let (bp, wp) = (bp as f64, wp as f64);
            let v = px[c] as f64;
            px[c] = ((v - bp) / (wp - bp) * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    Image {
        width: img.width,
        height: img.height,
        pixels,
    }
}

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum DistSortOrder {
    Intensity,
    Frequency,
}

/// Maps the image's distinct colors to evenly spaced gray levels chosen to
/// maximize contrast between them (like ppmdist). Alpha is untouched.
pub fn dist_grayscale(img: &Image, order: DistSortOrder) -> Image {
    use std::collections::HashMap;
    let mut counts: HashMap<[u8; 3], u64> = HashMap::new();
    for px in img.pixels.chunks_exact(4) {
        *counts.entry([px[0], px[1], px[2]]).or_insert(0) += 1;
    }

    let mut colors: Vec<([u8; 3], u64)> = counts.into_iter().collect();
    match order {
        DistSortOrder::Intensity => {
            colors.sort_by_key(|(c, _)| c[0] as u32 * 299 + c[1] as u32 * 587 + c[2] as u32 * 114);
        }
        DistSortOrder::Frequency => {
            colors.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        }
    }

    let n = colors.len();
    let mut gray_of: HashMap<[u8; 3], u8> = HashMap::new();
    for (i, (color, _)) in colors.into_iter().enumerate() {
        let gray = if n <= 1 {
            0
        } else {
            (i as f64 * 255.0 / (n - 1) as f64).round() as u8
        };
        gray_of.insert(color, gray);
    }

    let mut pixels = img.pixels.clone();
    for px in pixels.chunks_exact_mut(4) {
        let gray = gray_of[&[px[0], px[1], px[2]]];
        px[0] = gray;
        px[1] = gray;
        px[2] = gray;
    }
    Image {
        width: img.width,
        height: img.height,
        pixels,
    }
}

/// Reduces the image to at most `ncolors` distinct RGB colors via
/// median-cut quantization, remapping each pixel to its nearest palette
/// color (like pnmquant). Alpha is untouched.
pub fn quantize(img: &Image, ncolors: u32) -> anyhow::Result<Image> {
    ensure!(ncolors >= 1, "ncolors must be at least 1");
    use std::collections::HashMap;
    let mut counts: HashMap<[u8; 3], u64> = HashMap::new();
    for px in img.pixels.chunks_exact(4) {
        *counts.entry([px[0], px[1], px[2]]).or_insert(0) += 1;
    }
    // Sort deterministically so median-cut's tie-breaking (e.g. equal-range
    // axes) doesn't depend on HashMap iteration order.
    let mut weighted: Vec<([u8; 3], u64)> = counts.into_iter().collect();
    weighted.sort_by_key(|(c, _)| *c);
    let unique_colors: Vec<[u8; 3]> = weighted.iter().map(|(c, _)| *c).collect();

    let palette: Vec<[u8; 3]> = if unique_colors.len() <= ncolors as usize {
        unique_colors.clone()
    } else {
        median_cut(weighted, ncolors as usize)
    };

    let mut remap: HashMap<[u8; 3], [u8; 3]> = HashMap::new();
    for c in unique_colors {
        let nearest = *palette
            .iter()
            .min_by_key(|p| color_dist2(c, **p))
            .expect("palette is non-empty since ncolors >= 1 and the image has at least one pixel");
        remap.insert(c, nearest);
    }

    let mut pixels = img.pixels.clone();
    for px in pixels.chunks_exact_mut(4) {
        let mapped = remap[&[px[0], px[1], px[2]]];
        px[0] = mapped[0];
        px[1] = mapped[1];
        px[2] = mapped[2];
    }
    Ok(Image {
        width: img.width,
        height: img.height,
        pixels,
    })
}

fn color_dist2(a: [u8; 3], b: [u8; 3]) -> i32 {
    (0..3)
        .map(|i| {
            let d = a[i] as i32 - b[i] as i32;
            d * d
        })
        .sum()
}

fn axis_ranges(colors: &[([u8; 3], u64)]) -> [u32; 3] {
    let mut mins = [255u8; 3];
    let mut maxs = [0u8; 3];
    for (c, _) in colors {
        for i in 0..3 {
            mins[i] = mins[i].min(c[i]);
            maxs[i] = maxs[i].max(c[i]);
        }
    }
    [
        (maxs[0] - mins[0]) as u32,
        (maxs[1] - mins[1]) as u32,
        (maxs[2] - mins[2]) as u32,
    ]
}

fn median_cut(colors: Vec<([u8; 3], u64)>, ncolors: usize) -> Vec<[u8; 3]> {
    let mut boxes: Vec<Vec<([u8; 3], u64)>> = vec![colors];
    while boxes.len() < ncolors {
        let split_idx = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() > 1)
            .max_by_key(|(_, b)| *axis_ranges(b).iter().max().unwrap())
            .map(|(i, _)| i);
        let Some(split_idx) = split_idx else { break };
        let box_to_split = boxes.remove(split_idx);
        let ranges = axis_ranges(&box_to_split);
        let axis = (0..3usize).max_by_key(|&i| ranges[i]).unwrap();
        let (a, b) = split_box(box_to_split, axis);
        boxes.push(a);
        boxes.push(b);
    }
    boxes.iter().map(|b| weighted_average(b)).collect()
}

fn split_box(mut colors: Vec<([u8; 3], u64)>, axis: usize) -> (Vec<([u8; 3], u64)>, Vec<([u8; 3], u64)>) {
    colors.sort_by_key(|(c, _)| c[axis]);
    let total_weight: u64 = colors.iter().map(|(_, w)| w).sum();
    let mut cum = 0u64;
    let mut split_at = colors.len() / 2;
    for (i, (_, w)) in colors.iter().enumerate() {
        cum += w;
        if cum * 2 >= total_weight {
            split_at = i + 1;
            break;
        }
    }
    let split_at = split_at.clamp(1, colors.len() - 1);
    let rest = colors.split_off(split_at);
    (colors, rest)
}

fn weighted_average(colors: &[([u8; 3], u64)]) -> [u8; 3] {
    let total: u64 = colors.iter().map(|(_, w)| w).sum::<u64>().max(1);
    let mut sum = [0u64; 3];
    for (c, w) in colors {
        for i in 0..3 {
            sum[i] += c[i] as u64 * w;
        }
    }
    [(sum[0] / total) as u8, (sum[1] / total) as u8, (sum[2] / total) as u8]
}

fn rgb_distance(px: &[u8], background: [u8; 3]) -> f64 {
    let dr = px[0] as f64 - background[0] as f64;
    let dg = px[1] as f64 - background[1] as f64;
    let db = px[2] as f64 - background[2] as f64;
    (dr * dr + dg * dg + db * db).sqrt()
}

/// Computes an alpha channel via chroma-key: pixels within `similarity` of
/// `background` (RGB Euclidean distance) become transparent, pixels
/// farther than `similarity + blend` stay fully opaque, and the distance
/// range between the two ramps linearly — an anti-aliased edge instead of
/// a hard cutoff. Existing alpha is preserved (multiplied in); RGB is left
/// untouched (no spill/color decontamination).
pub fn chroma_key(img: &Image, background: [u8; 3], similarity: f64, blend: f64) -> Image {
    let mut pixels = img.pixels.clone();
    for px in pixels.chunks_exact_mut(4) {
        let dist = rgb_distance(px, background);
        let key_alpha = if dist <= similarity {
            0.0
        } else if dist >= similarity + blend {
            1.0
        } else {
            (dist - similarity) / blend
        };
        let existing_alpha = px[3] as f64 / 255.0;
        px[3] = (key_alpha * existing_alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    Image {
        width: img.width,
        height: img.height,
        pixels,
    }
}

/// Removes known-background color contamination ("spill") from
/// partially-transparent edge pixels, e.g. after [`chroma_key`]. Assumes
/// the observed color `C` is a blend `C = F*a + B*(1-a)` of the true
/// foreground color `F` and `background` `B` weighted by the existing
/// alpha `a`, and solves for `F`. Fully opaque/transparent pixels (where
/// there's nothing to reconstruct, or the reconstruction is numerically
/// meaningless) are left untouched.
pub fn decontaminate(img: &Image, background: [u8; 3]) -> Image {
    let mut pixels = img.pixels.clone();
    for px in pixels.chunks_exact_mut(4) {
        let a = px[3] as f64 / 255.0;
        if a <= 0.0 || a >= 1.0 {
            continue;
        }
        for c in 0..3 {
            let observed = px[c] as f64;
            let bg = background[c] as f64;
            let f = (observed - bg * (1.0 - a)) / a;
            px[c] = f.round().clamp(0.0, 255.0) as u8;
        }
    }
    Image {
        width: img.width,
        height: img.height,
        pixels,
    }
}

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum BlurChannels {
    All,
    Rgb,
    Alpha,
}

fn gaussian_kernel(sigma: f64) -> Vec<f64> {
    let radius = (sigma * 3.0).ceil().max(1.0) as i32;
    let mut kernel: Vec<f64> = (-radius..=radius)
        .map(|x| (-(x as f64 * x as f64) / (2.0 * sigma * sigma)).exp())
        .collect();
    let sum: f64 = kernel.iter().sum();
    for v in kernel.iter_mut() {
        *v /= sum;
    }
    kernel
}

/// Applies a separable Gaussian blur (edges clamped to the border pixel)
/// to the selected `channels`. Selecting [`BlurChannels::Alpha`] feathers
/// a mask (e.g. from [`chroma_key`]) without blurring its colors.
pub fn blur(img: &Image, sigma: f64, channels: BlurChannels) -> anyhow::Result<Image> {
    ensure!(sigma > 0.0, "sigma must be positive");
    let kernel = gaussian_kernel(sigma);
    let radius = (kernel.len() / 2) as i32;
    let chans: &[usize] = match channels {
        BlurChannels::All => &[0, 1, 2, 3],
        BlurChannels::Rgb => &[0, 1, 2],
        BlurChannels::Alpha => &[3],
    };

    let (w, h) = (img.width as i32, img.height as i32);
    let src: Vec<f64> = img.pixels.iter().map(|&v| v as f64).collect();

    let mut horiz = src.clone();
    for y in 0..h {
        for x in 0..w {
            for &c in chans {
                let mut acc = 0.0;
                for (k, &wgt) in kernel.iter().enumerate() {
                    let sx = (x + k as i32 - radius).clamp(0, w - 1);
                    acc += wgt * src[((y * w + sx) * 4) as usize + c];
                }
                horiz[((y * w + x) * 4) as usize + c] = acc;
            }
        }
    }

    let mut vert = horiz.clone();
    for y in 0..h {
        for x in 0..w {
            for &c in chans {
                let mut acc = 0.0;
                for (k, &wgt) in kernel.iter().enumerate() {
                    let sy = (y + k as i32 - radius).clamp(0, h - 1);
                    acc += wgt * horiz[((sy * w + x) * 4) as usize + c];
                }
                vert[((y * w + x) * 4) as usize + c] = acc;
            }
        }
    }

    let pixels: Vec<u8> = img
        .pixels
        .iter()
        .enumerate()
        .map(|(i, &orig)| {
            if chans.contains(&(i % 4)) {
                vert[i].round().clamp(0.0, 255.0) as u8
            } else {
                orig
            }
        })
        .collect();
    Ok(Image {
        width: img.width,
        height: img.height,
        pixels,
    })
}

/// Parses a CLI color argument of the form `"R,G,B"` or `"R,G,B,A"` (each
/// 0-255) into RGBA bytes. Shared by any tool that takes an explicit color
/// on the command line (e.g. an explicit background for `pngtrim`).
pub fn parse_color(s: &str) -> anyhow::Result<[u8; 4]> {
    let parts: Vec<&str> = s.split(',').collect();
    ensure!(
        parts.len() == 3 || parts.len() == 4,
        "color must be \"R,G,B\" or \"R,G,B,A\" (0-255 each), got {s:?}"
    );
    let r: u8 = parts[0].trim().parse()?;
    let g: u8 = parts[1].trim().parse()?;
    let b: u8 = parts[2].trim().parse()?;
    let a: u8 = if parts.len() == 4 { parts[3].trim().parse()? } else { 255 };
    Ok([r, g, b, a])
}

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum CompressionLevel {
    Fast,
    Default,
    Best,
}

impl From<CompressionLevel> for png::Compression {
    fn from(level: CompressionLevel) -> Self {
        match level {
            CompressionLevel::Fast => png::Compression::Fastest,
            CompressionLevel::Default => png::Compression::Balanced,
            CompressionLevel::Best => png::Compression::High,
        }
    }
}

/// `-v`/`--version` and `-q`/`--quiet`, shared by every netpng tool's
/// `Args` via `#[command(flatten)]` instead of each one redeclaring the
/// same two fields.
#[derive(clap::Args)]
pub struct CommonArgs {
    /// Print the version and exit.
    #[arg(short = 'v', long)]
    pub version: bool,
    /// Suppress informational messages (errors are still printed).
    #[arg(short = 'q', long)]
    pub quiet: bool,
}

impl CommonArgs {
    /// If `--version` was given, prints `"<name> <version>"` and returns
    /// `true` -- callers should return `Ok(())` immediately in that case.
    pub fn print_version_and_exit(&self, name: &str, version: &str) -> bool {
        if self.version {
            println!("{name} {version}");
        }
        self.version
    }
}

/// Rewrites netpbm-style single-dash common options to their two-dash
/// long form before clap sees them: `-version` -> `--version`,
/// `-quiet` -> `--quiet`. clap's derive API only accepts the two-dash
/// form for long options, so every tool funnels its raw argument
/// iterator through this before parsing; all other tokens pass through
/// unchanged.
pub fn normalize_args<I, T>(args: I) -> Vec<std::ffi::OsString>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString>,
{
    args.into_iter()
        .map(|arg| {
            let os: std::ffi::OsString = arg.into();
            match os.to_str() {
                Some("-version") => std::ffi::OsString::from("--version"),
                Some("-quiet") => std::ffi::OsString::from("--quiet"),
                _ => os,
            }
        })
        .collect()
}

/// Opens `path` for reading, treating the literal `"-"` as stdin.
/// Every netpng tool shares this convention.
pub fn open_input(path: &str) -> anyhow::Result<Box<dyn Read>> {
    if path == "-" {
        Ok(Box::new(io::stdin()))
    } else {
        Ok(Box::new(BufReader::new(File::open(path)?)))
    }
}

/// Opens `path` for writing, treating the literal `"-"` as stdout.
pub fn open_output(path: &str) -> anyhow::Result<Box<dyn Write>> {
    if path == "-" {
        Ok(Box::new(io::stdout()))
    } else {
        Ok(Box::new(BufWriter::new(File::create(path)?)))
    }
}

/// Reads an image from `path`: the literal `"clipboard"` reads whatever
/// image is currently on the system clipboard, `"-"` reads a PNG stream
/// from stdin, and anything else is read as a PNG file. This is the
/// convention every netpng tool's input should go through.
pub fn read_image(path: &str) -> anyhow::Result<Image> {
    if path == "clipboard" {
        read_clipboard_image()
    } else {
        decode_rgba8(open_input(path)?)
    }
}

/// Writes `img` to `path` under the same `"clipboard"` / `"-"` / file
/// convention as [`read_image`].
pub fn write_image(path: &str, img: &Image, level: CompressionLevel) -> anyhow::Result<()> {
    if path == "clipboard" {
        write_clipboard_image(img)
    } else {
        encode_rgba8(open_output(path)?, img, level)
    }
}

pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub color_type: png::ColorType,
    pub bit_depth: png::BitDepth,
    /// Size in bytes of the encoded source; `None` for "clipboard" (which
    /// has no compressed-file notion, only decoded pixels).
    pub source_bytes: Option<u64>,
}

/// Reads only the header metadata of a PNG (or clipboard image), without
/// decoding pixel data. Reports the *original* color type/bit depth,
/// unlike [`read_image`] which always normalizes to RGBA8.
pub fn read_info(path: &str) -> anyhow::Result<ImageInfo> {
    if path == "clipboard" {
        let img = read_clipboard_image()?;
        return Ok(ImageInfo {
            width: img.width,
            height: img.height,
            color_type: png::ColorType::Rgba,
            bit_depth: png::BitDepth::Eight,
            source_bytes: None,
        });
    }
    let mut bytes = Vec::new();
    open_input(path)?.read_to_end(&mut bytes)?;
    let source_bytes = bytes.len() as u64;
    let decoder = png::Decoder::new(io::Cursor::new(bytes));
    let reader = decoder.read_info()?;
    let info = reader.info();
    Ok(ImageInfo {
        width: info.width,
        height: info.height,
        color_type: info.color_type,
        bit_depth: info.bit_depth,
        source_bytes: Some(source_bytes),
    })
}

fn read_clipboard_image() -> anyhow::Result<Image> {
    let mut clipboard = arboard::Clipboard::new()?;
    let data = clipboard.get_image()?;
    Ok(Image {
        width: u32::try_from(data.width)?,
        height: u32::try_from(data.height)?,
        pixels: data.bytes.into_owned(),
    })
}

#[cfg(not(target_os = "linux"))]
fn write_clipboard_image(img: &Image) -> anyhow::Result<()> {
    let mut clipboard = arboard::Clipboard::new()?;
    clipboard.set_image(arboard::ImageData {
        width: img.width as usize,
        height: img.height as usize,
        bytes: std::borrow::Cow::Borrowed(&img.pixels),
    })?;
    Ok(())
}

/// X11/Wayland only serve clipboard content for as long as the setting
/// side keeps actively responding to selection requests; a plain
/// `set_image` call loses the content the instant it returns, regardless
/// of whether the calling *process* is still alive. `arboard`'s `wait()`
/// keeps responding, but it blocks the calling thread until something else
/// takes the clipboard (or forever) -- so it's run on a detached thread
/// here rather than the caller's, which would otherwise freeze (e.g. a
/// GUI event loop). That thread, and so the clipboard content, naturally
/// goes away when this process exits.
#[cfg(target_os = "linux")]
fn write_clipboard_image(img: &Image) -> anyhow::Result<()> {
    use arboard::SetExtLinux;

    let width = img.width as usize;
    let height = img.height as usize;
    let bytes = img.pixels.clone();
    std::thread::spawn(move || {
        let result = (|| -> anyhow::Result<()> {
            let mut clipboard = arboard::Clipboard::new()?;
            clipboard.set().wait().image(arboard::ImageData {
                width,
                height,
                bytes: std::borrow::Cow::Owned(bytes),
            })?;
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("netpng: clipboard-serving thread failed: {e}");
        }
    });
    Ok(())
}

pub fn decode_rgba8<R: Read>(mut r: R) -> anyhow::Result<Image> {
    // png::Decoder requires BufRead + Seek, which stdin/pipes don't offer.
    // Buffer the whole (single-image) input up front and decode from that.
    let mut bytes = Vec::new();
    r.read_to_end(&mut bytes)?;
    let mut decoder = png::Decoder::new(io::Cursor::new(bytes));
    // Expand palette/grayscale/tRNS and normalize bit depth to 8; the
    // remaining color-type differences (no alpha, grayscale) are handled
    // by hand below since this crate version's Transformations don't
    // guarantee a full RGBA expansion on their own.
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0u8; reader.output_buffer_size().ok_or_else(|| {
        anyhow::anyhow!("decoded image would not fit in memory on this machine")
    })?];
    let info = reader.next_frame(&mut buf)?;
    let bytes = &buf[..info.buffer_size()];
    let pixels = to_rgba8(bytes, info.color_type, info.width, info.height)?;
    Ok(Image {
        width: info.width,
        height: info.height,
        pixels,
    })
}

pub fn encode_rgba8<W: Write>(w: W, img: &Image, level: CompressionLevel) -> anyhow::Result<()> {
    ensure!(
        img.pixels.len() == (img.width as usize) * (img.height as usize) * 4,
        "image buffer length does not match width*height*4"
    );
    let mut encoder = png::Encoder::new(w, img.width, img.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(level.into());
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&img.pixels)?;
    Ok(())
}

fn to_rgba8(bytes: &[u8], color_type: png::ColorType, width: u32, height: u32) -> anyhow::Result<Vec<u8>> {
    let px = (width as usize) * (height as usize);
    let mut out = vec![0u8; px * 4];
    match color_type {
        png::ColorType::Rgba => {
            out.copy_from_slice(&bytes[..px * 4]);
        }
        png::ColorType::Rgb => {
            for i in 0..px {
                out[i * 4] = bytes[i * 3];
                out[i * 4 + 1] = bytes[i * 3 + 1];
                out[i * 4 + 2] = bytes[i * 3 + 2];
                out[i * 4 + 3] = 255;
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for i in 0..px {
                let g = bytes[i * 2];
                out[i * 4] = g;
                out[i * 4 + 1] = g;
                out[i * 4 + 2] = g;
                out[i * 4 + 3] = bytes[i * 2 + 1];
            }
        }
        png::ColorType::Grayscale => {
            for i in 0..px {
                let g = bytes[i];
                out[i * 4] = g;
                out[i * 4 + 1] = g;
                out[i * 4 + 2] = g;
                out[i * 4 + 3] = 255;
            }
        }
        png::ColorType::Indexed => {
            bail!("indexed color type should have been expanded by decode transformations");
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// JPEG bridge codec (lossy), shared by jpg2png and png2jpg
// ---------------------------------------------------------------------------

/// Decodes a JPEG from `path` (a file, or `"-"` for stdin) into RGBA8.
pub fn read_jpeg(path: &str) -> anyhow::Result<Image> {
    let mut bytes = Vec::new();
    open_input(path)?
        .read_to_end(&mut bytes)
        .context("reading JPEG input")?;
    let rgba = image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg)
        .map_err(|e| anyhow::anyhow!("input is not a JPEG: {e}"))?
        .to_rgba8();
    Ok(Image {
        width: rgba.width(),
        height: rgba.height(),
        pixels: rgba.into_raw(),
    })
}

/// Composites `img` (straight alpha) over an opaque `background` and returns
/// the RGB bytes, since JPEG has no alpha channel.
fn flatten_alpha(img: &Image, background: [u8; 3]) -> Vec<u8> {
    let mut out = vec![0u8; (img.width as usize) * (img.height as usize) * 3];
    for (o, px) in out.chunks_exact_mut(3).zip(img.pixels.chunks_exact(4)) {
        let a = px[3] as f32 / 255.0;
        for c in 0..3 {
            let blended = px[c] as f32 * a + background[c] as f32 * (1.0 - a);
            o[c] = blended.round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// Encodes `img` as a baseline JPEG (RGB, no alpha channel): the alpha
/// channel is first flattened onto `background`. `quality` is 1 (worst) to
/// 100 (best).
fn encode_jpeg(img: &Image, background: [u8; 3], quality: u8) -> anyhow::Result<Vec<u8>> {
    let rgb = flatten_alpha(img, background);
    let mut out = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    encoder
        .encode(&rgb, img.width, img.height, image::ExtendedColorType::Rgb8)
        .map_err(|e| anyhow::anyhow!("failed to encode JPEG: {e}"))?;
    Ok(out)
}

/// Flattens `img`'s alpha onto `background` and writes the result to `path`
/// as a baseline JPEG at `quality` (1 = worst, 100 = best).
pub fn write_jpeg(
    path: &str,
    img: &Image,
    background: [u8; 3],
    quality: u8,
) -> anyhow::Result<()> {
    let bytes = encode_jpeg(img, background, quality)?;
    let mut w = open_output(path)?;
    w.write_all(&bytes)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// ICO bridge codec, shared by ico2png and png2ico
// ---------------------------------------------------------------------------

fn read_u16_le(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn read_u32_le(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// Parses the ICO header/directory into a list of
/// `(width, height, data_offset, data_len)` for each stored image.
fn ico_directory(bytes: &[u8]) -> anyhow::Result<Vec<(u32, u32, usize, usize)>> {
    ensure!(bytes.len() >= 6, "not an ICO: too small for header");
    let kind = read_u16_le(bytes, 2);
    ensure!(kind == 1 || kind == 2, "not an ICO file (type {kind})");
    let count = read_u16_le(bytes, 4) as usize;
    ensure!(count >= 1, "ICO contains no images");
    ensure!(bytes.len() >= 6 + count * 16, "ICO directory truncated");

    let mut entries = Vec::new();
    for i in 0..count {
        let o = 6 + i * 16;
        // A width/height of 0 in the directory means 256.
        let w = match bytes[o] { 0 => 256, v => v as u32 };
        let h = match bytes[o + 1] { 0 => 256, v => v as u32 };
        let data_len = read_u32_le(bytes, o + 8) as usize;
        let data_offset = read_u32_le(bytes, o + 12) as usize;
        ensure!(
            data_offset + data_len <= bytes.len(),
            "ICO image data runs past end of file"
        );
        entries.push((w, h, data_offset, data_len));
    }
    Ok(entries)
}

const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Decodes one raw ICO image entry (the bytes that follow its 16-byte
/// directory record) into RGBA8. Entries are either a PNG (Vista+) or a BMP
/// DIB; both are handled.
fn decode_ico_image(data: &[u8]) -> anyhow::Result<Image> {
    if data.len() >= 8 && data[..8] == PNG_MAGIC {
        return decode_rgba8(io::Cursor::new(data));
    }
    decode_bmp_dib(data)
}

/// Decodes a BMP DIB (BITMAPINFOHEADER + bottom-up XOR mask + 1bpp AND mask)
/// as stored inside an ICO, into RGBA8. Supports 32/24/8 bpp.
fn decode_bmp_dib(data: &[u8]) -> anyhow::Result<Image> {
    ensure!(data.len() >= 40, "not an ICO: BMP DIB too small for header");
    let bi_size = read_u32_le(data, 0);
    ensure!(bi_size >= 40, "unsupported ICO BMP header size {bi_size}");
    let width = i32::from_le_bytes([data[4], data[5], data[6], data[7]]) as u32;
    // In an ICO, biHeight is twice the real height (XOR mask + AND mask).
    let bi_height = i32::from_le_bytes([data[8], data[9], data[10], data[11]]);
    let height = (bi_height / 2) as u32;
    let bit_count = read_u16_le(data, 14);
    let compression = read_u32_le(data, 16);
    ensure!(compression == 0, "ICO BMP DIB uses unsupported compression {compression}");
    ensure!(width > 0 && height > 0, "ICO BMP DIB has invalid dimensions");

    let xor_row_bytes = ((width as usize * bit_count as usize + 31) / 32) * 4;
    let xor_bytes = xor_row_bytes * height as usize;
    let and_row_bytes = ((width as usize + 31) / 32) * 4;
    let and_bytes = and_row_bytes * height as usize;

    let palette_count = if bit_count <= 8 { 1 << bit_count } else { 0 };
    let palette_bytes = palette_count * 4;
    let xor_start = 40 + palette_bytes;
    ensure!(
        xor_start + xor_bytes + and_bytes <= data.len(),
        "ICO BMP DIB pixel data truncated"
    );

    // Palette (8bpp) is an array of BGRA quadruples right after the header.
    let mut palette: Vec<[u8; 4]> = Vec::new();
    for i in 0..palette_count {
        let o = 40 + i * 4;
        palette.push([data[o + 2], data[o + 1], data[o], 255]); // BGRA -> RGBA
    }

    let and = &data[xor_start + xor_bytes..xor_start + xor_bytes + and_bytes];
    let mut pixels = vec![0u8; (width as usize) * (height as usize) * 4];
    for y in 0..height {
        // Bottom-up: the first row in the file is the image's bottom row.
        let file_row = (height - 1 - y) as usize;
        let xor_row = &data
            [xor_start + file_row * xor_row_bytes..xor_start + (file_row + 1) * xor_row_bytes];
        let and_row = &and[file_row * and_row_bytes..file_row * and_row_bytes + and_row_bytes];
        for x in 0..width {
            let (r, g, b, a) = match bit_count {
                32 => {
                    let o = (x * 4) as usize;
                    (xor_row[o + 2], xor_row[o + 1], xor_row[o], xor_row[o + 3])
                }
                24 => {
                    let o = (x * 3) as usize;
                    let a = if (and_row[x as usize / 8] >> (x % 8)) & 1 == 1 { 0 } else { 255 };
                    (xor_row[o + 2], xor_row[o + 1], xor_row[o], a)
                }
                8 => {
                    let a = if (and_row[x as usize / 8] >> (x % 8)) & 1 == 1 { 0 } else { 255 };
                    let [r, g, b, _] = palette[xor_row[x as usize] as usize];
                    (r, g, b, a)
                }
                other => bail!("unsupported ICO BMP bit depth {other}"),
            };
            let p = ((y * width + x) * 4) as usize;
            pixels[p] = r;
            pixels[p + 1] = g;
            pixels[p + 2] = b;
            pixels[p + 3] = a;
        }
    }
    Ok(Image {
        width,
        height,
        pixels,
    })
}

/// Lists the `(width, height)` of each image stored in the ICO at `path`.
pub fn ico_sizes(path: &str) -> anyhow::Result<Vec<(u32, u32)>> {
    let mut bytes = Vec::new();
    open_input(path)?.read_to_end(&mut bytes)?;
    Ok(ico_directory(&bytes)?
        .into_iter()
        .map(|(w, h, _, _)| (w, h))
        .collect())
}

/// Reads one image from the ICO at `path` into RGBA8. When `target` is
/// `None` the largest image (by area) is used; otherwise the entry whose
/// width is closest to `target` is chosen.
pub fn read_ico(path: &str, target: Option<u32>) -> anyhow::Result<Image> {
    let mut bytes = Vec::new();
    open_input(path)?.read_to_end(&mut bytes)?;
    let entries = ico_directory(&bytes)?;
    let idx = match target {
        None => entries
            .iter()
            .enumerate()
            .max_by_key(|(_, (w, h, _, _))| w.saturating_mul(*h))
            .map(|(i, _)| i)
            .ok_or_else(|| anyhow::anyhow!("ICO contains no images"))?,
        Some(t) => entries
            .iter()
            .enumerate()
            .min_by_key(|(_, (w, _, _, _))| w.abs_diff(t))
            .map(|(i, _)| i)
            .ok_or_else(|| anyhow::anyhow!("ICO contains no images"))?,
    };
    let (_, _, data_offset, data_len) = entries[idx];
    decode_ico_image(&bytes[data_offset..data_offset + data_len])
}

/// Scales `img` so it fits inside a `size` x `size` square (aspect ratio
/// preserved, bilinear) and centers it on a transparent canvas.
fn fit_into_square(img: &Image, size: u32) -> Image {
    let (iw, ih) = (img.width as f64, img.height as f64);
    let f = (size as f64 / iw).min(size as f64 / ih);
    let nw = (iw * f).round().max(1.0) as u32;
    let nh = (ih * f).round().max(1.0) as u32;
    let scaled = if nw == img.width && nh == img.height {
        img.clone()
    } else {
        scale(img, nw, nh)
    };
    let mut out = Image {
        width: size,
        height: size,
        pixels: vec![0u8; (size as usize) * (size as usize) * 4],
    };
    let ox = (size - nw) / 2;
    let oy = (size - nh) / 2;
    for y in 0..nh {
        let src = (y * nw * 4) as usize;
        let dst = (((oy + y) * size + ox) * 4) as usize;
        out.pixels[dst..dst + (nw * 4) as usize]
            .copy_from_slice(&scaled.pixels[src..src + (nw * 4) as usize]);
    }
    out
}

/// Encodes `img` (RGBA8) as a BMP DIB (BITMAPINFOHEADER + bottom-up BGRA XOR
/// mask + 1bpp AND mask) as stored inside an ICO.
fn encode_bmp_dib(img: &Image) -> Vec<u8> {
    let (w, h) = (img.width, img.height);
    let xor_row_bytes = w as usize * 4; // 32bpp, already 4-byte aligned
    let xor_bytes = xor_row_bytes * h as usize;
    let and_row_bytes = ((w as usize + 31) / 32) * 4;
    let and_bytes = and_row_bytes * h as usize;

    let mut out = Vec::new();
    out.extend_from_slice(&40u32.to_le_bytes()); // biSize
    out.extend_from_slice(&(w as i32).to_le_bytes()); // biWidth
    out.extend_from_slice(&((h as i32) * 2).to_le_bytes()); // biHeight = XOR + AND
    out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    out.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    out.extend_from_slice(&0u32.to_le_bytes()); // biCompression = BI_RGB
    out.extend_from_slice(&((xor_bytes + and_bytes) as u32).to_le_bytes()); // biSizeImage
    out.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
    out.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
    out.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
    out.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant

    // XOR mask: bottom-up BGRA.
    for y in (0..h).rev() {
        for x in 0..w {
            let p = ((y * w + x) * 4) as usize;
            out.push(img.pixels[p + 2]); // B
            out.push(img.pixels[p + 1]); // G
            out.push(img.pixels[p]); // R
            out.push(img.pixels[p + 3]); // A
        }
    }

    // AND mask: bottom-up 1bpp, bit set where fully transparent.
    for y in (0..h).rev() {
        for byte_i in 0..and_row_bytes {
            let mut byte = 0u8;
            for bit in 0..8 {
                let x = (byte_i * 8 + bit) as u32;
                if x < w && img.pixels[((y * w + x) * 4 + 3) as usize] == 0 {
                    byte |= 1 << bit;
                }
            }
            out.push(byte);
        }
    }
    out
}

/// Builds an ICO in memory with one entry per size in `sizes`. Each entry is
/// `img` scaled to fit that square; sizes below 256 are stored as 32bpp BMP
/// DIBs and 256 as a PNG (readable by Vista+).
fn encode_ico(img: &Image, sizes: &[u32]) -> anyhow::Result<Vec<u8>> {
    ensure!(!sizes.is_empty(), "no output sizes given");
    for &s in sizes {
        ensure!(s >= 1 && s <= 256, "ICO size {s} out of range 1-256");
    }
    let mut sizes: Vec<u32> = sizes.iter().copied().collect();
    sizes.sort_unstable();
    sizes.dedup();

    let mut encoded: Vec<(u32, Vec<u8>)> = Vec::new();
    for &s in &sizes {
        let fitted = fit_into_square(img, s);
        let bytes = if s == 256 {
            let mut png = Vec::new();
            encode_rgba8(&mut png, &fitted, CompressionLevel::Default)?;
            png
        } else {
            encode_bmp_dib(&fitted)
        };
        encoded.push((s, bytes));
    }

    let count = encoded.len() as u16;
    let dir_size = 6 + encoded.len() * 16;
    let mut offset = dir_size;
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // type = icon
    out.extend_from_slice(&count.to_le_bytes());
    for (s, bytes) in &encoded {
        // Directory width/height of 0 means 256.
        let dim = if *s == 256 { 0 } else { *s as u8 };
        out.push(dim);
        out.push(dim);
        out.push(0); // color count
        out.push(0); // reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bit count
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += bytes.len();
    }
    for (_, bytes) in &encoded {
        out.extend_from_slice(bytes);
    }
    Ok(out)
}

/// Writes `img` to `path` as an ICO with one entry per size in `sizes`.
pub fn write_ico(path: &str, img: &Image, sizes: &[u32]) -> anyhow::Result<()> {
    let bytes = encode_ico(img, sizes)?;
    let mut w = open_output(path)?;
    w.write_all(&bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let img = Image {
            width: 2,
            height: 2,
            pixels: vec![
                255, 0, 0, 255, //
                0, 255, 0, 255, //
                0, 0, 255, 255, //
                255, 255, 255, 255, //
            ],
        };
        let mut buf = Vec::new();
        encode_rgba8(&mut buf, &img, CompressionLevel::Fast).unwrap();
        let decoded = decode_rgba8(buf.as_slice()).unwrap();
        assert_eq!(decoded.width, img.width);
        assert_eq!(decoded.height, img.height);
        assert_eq!(decoded.pixels, img.pixels);
    }

    #[test]
    fn composite_over_blends_half_alpha() {
        let base = Image {
            width: 1,
            height: 1,
            pixels: vec![255, 0, 0, 255], // opaque red
        };
        let overlay = Image {
            width: 1,
            height: 1,
            pixels: vec![0, 255, 0, 128], // ~50% alpha green
        };
        let out = composite_over(&base, &overlay, 0, 0);
        // Over opaque red, ~50% green should land near [127, 128, 0, 255].
        for (got, expected) in out.pixels.iter().zip([127u8, 128, 0, 255].iter()) {
            assert!(got.abs_diff(*expected) <= 1, "got {:?}", out.pixels);
        }
    }

    #[test]
    fn composite_over_clips_out_of_bounds_overlay() {
        let base = Image {
            width: 2,
            height: 2,
            pixels: vec![1, 2, 3, 255, 1, 2, 3, 255, 1, 2, 3, 255, 1, 2, 3, 255],
        };
        let overlay = Image {
            width: 2,
            height: 2,
            pixels: vec![255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255],
        };
        // Fully off-canvas: base must come back unchanged.
        let out = composite_over(&base, &overlay, 10, 10);
        assert_eq!(out.pixels, base.pixels);
    }

    fn gray(v: u8) -> [u8; 4] {
        [v, v, v, 255]
    }

    #[test]
    fn flip_rotate90_on_2x1() {
        // A at (0,0), B at (1,0) in a 2-wide, 1-tall image.
        let img = Image {
            width: 2,
            height: 1,
            pixels: [gray(1), gray(2)].concat(),
        };
        let out = flip(&img, FlipMode::Rotate90);
        assert_eq!((out.width, out.height), (1, 2));
        assert_eq!(pixel_at(&out, 0, 0), gray(1));
        assert_eq!(pixel_at(&out, 0, 1), gray(2));
    }

    #[test]
    fn flip_rotate270_on_2x1() {
        let img = Image {
            width: 2,
            height: 1,
            pixels: [gray(1), gray(2)].concat(),
        };
        let out = flip(&img, FlipMode::Rotate270);
        assert_eq!((out.width, out.height), (1, 2));
        assert_eq!(pixel_at(&out, 0, 0), gray(2));
        assert_eq!(pixel_at(&out, 0, 1), gray(1));
    }

    #[test]
    fn flip_horizontal_and_vertical() {
        let img = Image {
            width: 2,
            height: 2,
            pixels: [gray(1), gray(2), gray(3), gray(4)].concat(), // row0: 1,2 row1: 3,4
        };
        let h = flip(&img, FlipMode::Horizontal);
        assert_eq!(pixel_at(&h, 0, 0), gray(2));
        assert_eq!(pixel_at(&h, 1, 0), gray(1));
        let v = flip(&img, FlipMode::Vertical);
        assert_eq!(pixel_at(&v, 0, 0), gray(3));
        assert_eq!(pixel_at(&v, 0, 1), gray(1));
    }

    #[test]
    fn quantize_noop_when_colors_fit() {
        let img = Image {
            width: 2,
            height: 1,
            pixels: [gray(10), gray(200)].concat(),
        };
        let out = quantize(&img, 8).unwrap();
        assert_eq!(out.pixels, img.pixels);
    }

    #[test]
    fn quantize_to_one_color_averages() {
        let img = Image {
            width: 2,
            height: 1,
            pixels: [gray(0), gray(100)].concat(),
        };
        let out = quantize(&img, 1).unwrap();
        assert_eq!(pixel_at(&out, 0, 0), pixel_at(&out, 1, 0));
    }

    #[test]
    fn dist_grayscale_spreads_two_colors_to_extremes() {
        let img = Image {
            width: 2,
            height: 1,
            pixels: vec![10, 20, 30, 255, 200, 210, 220, 255],
        };
        let out = dist_grayscale(&img, DistSortOrder::Intensity);
        let mut grays: Vec<u8> = out.pixels.chunks_exact(4).map(|p| p[0]).collect();
        grays.sort();
        assert_eq!(grays, vec![0, 255]);
    }

    #[test]
    fn chroma_key_hits_endpoints() {
        let bg = [0u8, 255, 0];
        let at_bg = Image {
            width: 1,
            height: 1,
            pixels: vec![0, 255, 0, 255],
        };
        assert_eq!(pixel_at(&chroma_key(&at_bg, bg, 10.0, 30.0), 0, 0)[3], 0);

        let far = Image {
            width: 1,
            height: 1,
            pixels: vec![255, 0, 0, 255],
        };
        assert_eq!(pixel_at(&chroma_key(&far, bg, 10.0, 30.0), 0, 0)[3], 255);
    }

    #[test]
    fn chroma_key_ramps_linearly_in_transition_band() {
        // Green shifted by (20,0,0) is RGB-distance 20 from pure green.
        let bg = [0u8, 255, 0];
        let img = Image {
            width: 1,
            height: 1,
            pixels: vec![20, 255, 0, 255],
        };
        // similarity=10, blend=30: ramp covers distance 10..40; at dist=20
        // that's 1/3 of the way, so alpha should land near 85.
        let out = chroma_key(&img, bg, 10.0, 30.0);
        assert!(pixel_at(&out, 0, 0)[3].abs_diff(85) <= 2);
    }

    #[test]
    fn chroma_key_preserves_existing_alpha() {
        let bg = [0u8, 255, 0];
        let img = Image {
            width: 1,
            height: 1,
            pixels: vec![255, 0, 0, 128], // far from bg, but already half-transparent
        };
        let out = chroma_key(&img, bg, 10.0, 30.0);
        assert_eq!(pixel_at(&out, 0, 0)[3], 128);
    }

    #[test]
    fn decontaminate_recovers_foreground_color() {
        // Pure red foreground at ~50% coverage over green background:
        // observed = red*0.5 + green*0.5 = (128, 128, 0).
        let bg = [0u8, 255, 0];
        let img = Image {
            width: 1,
            height: 1,
            pixels: vec![128, 128, 0, 128],
        };
        let out = decontaminate(&img, bg);
        let px = pixel_at(&out, 0, 0);
        assert!(px[0].abs_diff(255) <= 2, "R: {px:?}");
        assert!(px[1] <= 3, "G: {px:?}");
        assert!(px[2] <= 2, "B: {px:?}");
        assert_eq!(px[3], 128, "alpha must be untouched");
    }

    #[test]
    fn decontaminate_leaves_opaque_and_transparent_pixels_untouched() {
        let bg = [0u8, 255, 0];
        let img = Image {
            width: 2,
            height: 1,
            pixels: vec![10, 20, 30, 255, 40, 50, 60, 0],
        };
        let out = decontaminate(&img, bg);
        assert_eq!(out.pixels, img.pixels);
    }

    #[test]
    fn blur_uniform_image_is_unchanged() {
        let img = Image {
            width: 5,
            height: 5,
            pixels: [gray(42); 25].concat(),
        };
        let out = blur(&img, 2.0, BlurChannels::All).unwrap();
        for px in out.pixels.chunks_exact(4) {
            assert_eq!(px, [42, 42, 42, 255]);
        }
    }

    #[test]
    fn blur_alpha_only_leaves_rgb_untouched() {
        let mut pixels = vec![0u8; 5 * 4];
        pixels[0] = 10; // R of first pixel
        pixels[3] = 255; // A of first pixel
        pixels[3 + 4] = 0; // A of second pixel is 0, rest default 0
        let img = Image {
            width: 5,
            height: 1,
            pixels,
        };
        let out = blur(&img, 1.0, BlurChannels::Alpha).unwrap();
        assert_eq!(out.pixels[0], 10, "RGB must be untouched by alpha-only blur");
        // Alpha should have spread from the single 255 pixel into its neighbor.
        assert!(out.pixels[3 + 4] > 0, "alpha should have blurred into the neighbor");
    }

    #[test]
    fn jpeg_round_trip_flat_color_is_close() {
        let img = Image {
            width: 16,
            height: 16,
            pixels: vec![200, 120, 40, 255].repeat(16 * 16),
        };
        let bytes = encode_jpeg(&img, [255; 3], 95).unwrap();
        let rgba = image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg)
            .unwrap()
            .to_rgba8();
        assert_eq!((rgba.width(), rgba.height()), (16, 16));
        for px in rgba.pixels() {
            assert!((px[0] as i32 - 200).abs() <= 8, "R: {px:?}");
            assert!((px[1] as i32 - 120).abs() <= 8, "G: {px:?}");
            assert!((px[2] as i32 - 40).abs() <= 8, "B: {px:?}");
            assert_eq!(px[3], 255);
        }
    }

    #[test]
    fn jpeg_encode_flattens_alpha_onto_background() {
        let img = Image {
            width: 1,
            height: 1,
            pixels: vec![255, 0, 0, 128], // ~50% red
        };
        let rgb = flatten_alpha(&img, [0, 0, 255]);
        for (got, want) in rgb.iter().zip([128u8, 0, 127].iter()) {
            assert!(got.abs_diff(*want) <= 1, "got {rgb:?}");
        }
    }

    #[test]
    fn ico_bmp32_round_trip_is_lossless() {
        let mut pixels = Vec::new();
        for y in 0..32u32 {
            for x in 0..32u32 {
                pixels.extend_from_slice(&[x as u8, y as u8, 128, 255 - ((x + y) % 256) as u8]);
            }
        }
        let img = Image {
            width: 32,
            height: 32,
            pixels,
        };
        let bytes = encode_ico(&img, &[32]).unwrap();
        let entries = ico_directory(&bytes).unwrap();
        assert_eq!(entries.len(), 1);
        let (w, h, off, len) = entries[0];
        assert_eq!((w, h), (32, 32));
        let decoded = decode_ico_image(&bytes[off..off + len]).unwrap();
        assert_eq!(decoded.width, 32);
        assert_eq!(decoded.height, 32);
        assert_eq!(decoded.pixels, img.pixels);
    }

    #[test]
    fn ico_256_entry_is_png_compressed() {
        let img = Image {
            width: 256,
            height: 256,
            pixels: vec![10, 20, 30, 255].repeat(256 * 256),
        };
        let bytes = encode_ico(&img, &[256]).unwrap();
        let (w, h, off, len) = ico_directory(&bytes).unwrap()[0];
        assert_eq!((w, h), (256, 256));
        let data = &bytes[off..off + len];
        assert_eq!(&data[..8], &PNG_MAGIC, "256px entry must be PNG-compressed");
        let decoded = decode_ico_image(data).unwrap();
        assert_eq!(decoded.pixels, img.pixels);
    }

    #[test]
    fn ico_non_square_source_is_centered_on_transparent_square() {
        let img = Image {
            width: 4,
            height: 2,
            pixels: vec![7, 7, 7, 255].repeat(8),
        };
        let bytes = encode_ico(&img, &[4]).unwrap();
        let (w, h, off, len) = ico_directory(&bytes).unwrap()[0];
        assert_eq!((w, h), (4, 4));
        let decoded = decode_ico_image(&bytes[off..off + len]).unwrap();
        assert_eq!(pixel_at(&decoded, 0, 0)[3], 0, "top padding row must be transparent");
        assert_eq!(pixel_at(&decoded, 0, 1), [7, 7, 7, 255]);
        assert_eq!(pixel_at(&decoded, 0, 2), [7, 7, 7, 255]);
        assert_eq!(pixel_at(&decoded, 0, 3)[3], 0, "bottom padding row must be transparent");
    }

    #[test]
    fn read_ico_picks_largest_by_default_and_nearest_for_target() {
        let img = Image {
            width: 64,
            height: 64,
            pixels: vec![42, 42, 42, 255].repeat(64 * 64),
        };
        let dir = std::env::temp_dir().join(format!("netpng-ico-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("multi.ico");
        let p = path.to_str().unwrap();
        write_ico(p, &img, &[16, 48, 32]).unwrap();
        let largest = read_ico(p, None).unwrap();
        assert_eq!((largest.width, largest.height), (48, 48));
        let nearest = read_ico(p, Some(20)).unwrap();
        assert_eq!((nearest.width, nearest.height), (16, 16));
        let exact = read_ico(p, Some(32)).unwrap();
        assert_eq!((exact.width, exact.height), (32, 32));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ico_directory_rejects_garbage() {
        assert!(ico_directory(&[0u8; 4]).is_err(), "too small");
        assert!(ico_directory(&[0, 0, 3, 0, 0, 0]).is_err(), "wrong file type");
        assert!(ico_directory(&[0, 0, 1, 0, 0, 0]).is_err(), "zero entries");
        // Two entries advertised, but no room for the directory records.
        assert!(ico_directory(&[0, 0, 1, 0, 2, 0, 1, 1, 0, 0, 0, 0]).is_err());
    }
}
