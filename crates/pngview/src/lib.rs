use clap::Parser;
use minifb::{Key, Window, WindowOptions};
use netpng_core::{read_image, Image};

/// Display an image in a window (reads from a pipe, a file, or the
/// clipboard). Transparent areas are shown over a checkerboard. Press Esc
/// or close the window to exit.
#[derive(Parser)]
struct Args {
    /// Input path, "-" for stdin, or "clipboard".
    #[arg(long, default_value = "-")]
    input: String,
}

pub fn run<I, T>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let args = Args::parse_from(args);
    let img = read_image(&args.input)?;
    let buffer = composite_checkerboard(&img);

    let mut window = Window::new(
        "pngview",
        img.width as usize,
        img.height as usize,
        WindowOptions::default(),
    )?;
    window.set_target_fps(30);

    while window.is_open() && !window.is_key_down(Key::Escape) {
        window.update_with_buffer(&buffer, img.width as usize, img.height as usize)?;
    }
    Ok(())
}

fn composite_checkerboard(img: &Image) -> Vec<u32> {
    const TILE: u32 = 8;
    const LIGHT: f64 = 205.0;
    const DARK: f64 = 155.0;

    let mut buffer = Vec::with_capacity((img.width as usize) * (img.height as usize));
    for y in 0..img.height {
        for x in 0..img.width {
            let i = ((y * img.width + x) * 4) as usize;
            let (r, g, b, a) = (
                img.pixels[i] as f64,
                img.pixels[i + 1] as f64,
                img.pixels[i + 2] as f64,
                img.pixels[i + 3] as f64 / 255.0,
            );
            let checker = if ((x / TILE) + (y / TILE)) % 2 == 0 { LIGHT } else { DARK };
            let out_r = (r * a + checker * (1.0 - a)).round().clamp(0.0, 255.0) as u32;
            let out_g = (g * a + checker * (1.0 - a)).round().clamp(0.0, 255.0) as u32;
            let out_b = (b * a + checker * (1.0 - a)).round().clamp(0.0, 255.0) as u32;
            buffer.push((out_r << 16) | (out_g << 8) | out_b);
        }
    }
    buffer
}
