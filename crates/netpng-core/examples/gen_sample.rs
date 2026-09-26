use netpng_core::{encode_rgba8, open_output, CompressionLevel, Image};

/// Writes a small quadrant-colored RGBA PNG, for manually smoke-testing
/// the other netpng tools without needing an external sample file.
fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .expect("usage: gen_sample <output.png>");

    let (w, h) = (8u32, 8u32);
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let (r, g, b) = match (x < w / 2, y < h / 2) {
                (true, true) => (255, 0, 0),
                (false, true) => (0, 255, 0),
                (true, false) => (0, 0, 255),
                (false, false) => (255, 255, 0),
            };
            let i = ((y * w + x) * 4) as usize;
            pixels[i] = r;
            pixels[i + 1] = g;
            pixels[i + 2] = b;
            pixels[i + 3] = 255;
        }
    }

    let img = Image {
        width: w,
        height: h,
        pixels,
    };
    encode_rgba8(open_output(&path)?, &img, CompressionLevel::Best)
}
