use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};

use anyhow::{bail, ensure};

/// An image decoded into 8-bit RGBA, regardless of the source PNG's
/// original color type or bit depth. This is the common currency every
/// netpng tool passes between decode and encode.
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
/// convention every netpng tool's `--input` should go through.
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

fn write_clipboard_image(img: &Image) -> anyhow::Result<()> {
    let mut clipboard = arboard::Clipboard::new()?;
    clipboard.set_image(arboard::ImageData {
        width: img.width as usize,
        height: img.height as usize,
        bytes: std::borrow::Cow::Borrowed(&img.pixels),
    })?;
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
}
