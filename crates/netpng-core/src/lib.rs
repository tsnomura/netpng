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
}
