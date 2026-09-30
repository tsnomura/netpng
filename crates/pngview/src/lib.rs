use clap::Parser;
use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};
use netpng_core::{read_image, write_image, CompressionLevel, Image};

/// Display an image in a window (reads from a pipe, a file, or the
/// clipboard). Transparent areas are shown over a checkerboard.
///
/// Controls: mouse wheel zooms (centered on the cursor); left-click-drag
/// pans; Space cycles zoom between --zoom, actual size (100%), and fit-to-
/// window; Enter re-centers the image; "0" resets zoom/pan; "s" saves the
/// image to a timestamped PNG in the current directory; the window is
/// resizable; Esc or "q" or closing the window exits.
#[derive(Parser)]
struct Args {
    /// Input path, "-" for stdin, or "clipboard".
    #[arg(long, default_value = "-")]
    input: String,
    /// Zoom level Space cycles to as its first step (2.0 = 200%).
    #[arg(long, default_value_t = 2.0)]
    zoom: f64,
}

const MIN_ZOOM: f64 = 0.05;
const MAX_ZOOM: f64 = 32.0;
/// Zoom multiplier per unit of scroll-wheel delta; smaller = gentler.
const ZOOM_WHEEL_FACTOR: f64 = 1.01;
/// Small images (icons, sprites, our own test fixtures...) would otherwise
/// open in a window as tiny as the image itself; scale up so the window's
/// larger side is at least this many pixels.
const MIN_INITIAL_WINDOW: f64 = 400.0;
/// Color for area outside the image canvas (panned/zoomed past its edge),
/// distinct from the checkerboard used for transparent pixels *within* it.
const OUTSIDE_CANVAS: u32 = 0x00303030;

pub fn run<I, T>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let args = Args::parse_from(args);
    let img = read_image(&args.input)?;
    let checkerboarded = composite_checkerboard(&img);

    eprintln!(
        "pngview: wheel = zoom, left-drag = pan, space = cycle zoom, enter = center, \
         0 = reset, s = save, Esc/q = quit"
    );

    let larger_side = (img.width.max(img.height) as f64).max(1.0);
    let initial_zoom = (MIN_INITIAL_WINDOW / larger_side).max(1.0);
    let initial_win_w = ((img.width as f64) * initial_zoom).round() as usize;
    let initial_win_h = ((img.height as f64) * initial_zoom).round() as usize;

    let mut window = Window::new(
        "pngview",
        initial_win_w,
        initial_win_h,
        WindowOptions {
            resize: true,
            ..WindowOptions::default()
        },
    )?;
    window.set_target_fps(60);

    let mut zoom: f64 = initial_zoom;
    let mut pan_x: f64 = 0.0;
    let mut pan_y: f64 = 0.0;
    let mut last_mouse: Option<(f32, f32)> = None;
    let mut zoom_cycle: u8 = 0;

    while window.is_open() && !window.is_key_down(Key::Escape) && !window.is_key_down(Key::Q) {
        let (win_w, win_h) = window.get_size();
        let (win_w, win_h) = (win_w.max(1), win_h.max(1));

        if let Some((_, wheel_dy)) = window.get_scroll_wheel() {
            if wheel_dy.abs() > f32::EPSILON {
                let old_zoom = zoom;
                zoom = (zoom * ZOOM_WHEEL_FACTOR.powf(wheel_dy as f64)).clamp(MIN_ZOOM, MAX_ZOOM);
                if let Some((mx, my)) = window.get_mouse_pos(MouseMode::Clamp) {
                    // Keep the image point under the cursor fixed on screen.
                    let anchor_x = pan_x + mx as f64 / old_zoom;
                    let anchor_y = pan_y + my as f64 / old_zoom;
                    pan_x = anchor_x - mx as f64 / zoom;
                    pan_y = anchor_y - my as f64 / zoom;
                }
            }
        }

        if window.get_mouse_down(MouseButton::Left) {
            if let Some((mx, my)) = window.get_mouse_pos(MouseMode::Pass) {
                if let Some((last_x, last_y)) = last_mouse {
                    pan_x -= (mx - last_x) as f64 / zoom;
                    pan_y -= (my - last_y) as f64 / zoom;
                }
                last_mouse = Some((mx, my));
            }
        } else {
            last_mouse = None;
        }

        if window.is_key_pressed(Key::Key0, KeyRepeat::No) {
            zoom = 1.0;
            pan_x = 0.0;
            pan_y = 0.0;
            zoom_cycle = 0;
        }

        if window.is_key_pressed(Key::Space, KeyRepeat::No) {
            zoom = match zoom_cycle {
                0 => args.zoom,
                1 => 1.0,
                _ => fit_zoom(img.width, img.height, win_w, win_h),
            }
            .clamp(MIN_ZOOM, MAX_ZOOM);
            zoom_cycle = (zoom_cycle + 1) % 3;
            (pan_x, pan_y) = centered_pan(img.width, img.height, win_w, win_h, zoom);
        }

        if window.is_key_pressed(Key::Enter, KeyRepeat::No) {
            (pan_x, pan_y) = centered_pan(img.width, img.height, win_w, win_h, zoom);
        }

        if window.is_key_pressed(Key::S, KeyRepeat::No) {
            let path = next_save_path();
            match write_image(&path.to_string_lossy(), &img, CompressionLevel::Default) {
                Ok(()) => eprintln!("pngview: saved to {}", path.display()),
                Err(e) => eprintln!("pngview: failed to save: {e}"),
            }
        }

        let view = render_view(&checkerboarded, img.width, img.height, win_w, win_h, zoom, pan_x, pan_y);
        window.update_with_buffer(&view, win_w, win_h)?;
    }
    Ok(())
}

/// The largest zoom that fits the whole image within `win_w` x `win_h`.
fn fit_zoom(img_w: u32, img_h: u32, win_w: usize, win_h: usize) -> f64 {
    (win_w as f64 / img_w as f64).min(win_h as f64 / img_h as f64)
}

/// Pan that centers the image within `win_w` x `win_h` at the given zoom.
fn centered_pan(img_w: u32, img_h: u32, win_w: usize, win_h: usize, zoom: f64) -> (f64, f64) {
    let pan_x = img_w as f64 / 2.0 - win_w as f64 / (2.0 * zoom);
    let pan_y = img_h as f64 / 2.0 - win_h as f64 / (2.0 * zoom);
    (pan_x, pan_y)
}

/// A fresh, never-yet-used "pngview-save-<unix seconds>[-N].png" path in
/// the current directory.
fn next_save_path() -> std::path::PathBuf {
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut n = 0u32;
    loop {
        let name = if n == 0 {
            format!("pngview-save-{epoch}.png")
        } else {
            format!("pngview-save-{epoch}-{n}.png")
        };
        let path = std::path::PathBuf::from(name);
        if !path.exists() {
            return path;
        }
        n += 1;
    }
}

/// Resamples (nearest-neighbor) the precomputed, checkerboard-composited
/// image into a `win_w` x `win_h` view at the given `zoom`/pan, so the
/// window can be any size independent of the image's own resolution.
fn render_view(
    image: &[u32],
    img_w: u32,
    img_h: u32,
    win_w: usize,
    win_h: usize,
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
) -> Vec<u32> {
    let mut view = vec![OUTSIDE_CANVAS; win_w * win_h];
    for oy in 0..win_h {
        let src_y = pan_y + oy as f64 / zoom;
        if src_y < 0.0 || src_y >= img_h as f64 {
            continue;
        }
        let sy = src_y as u32;
        for ox in 0..win_w {
            let src_x = pan_x + ox as f64 / zoom;
            if src_x < 0.0 || src_x >= img_w as f64 {
                continue;
            }
            let sx = src_x as u32;
            view[oy * win_w + ox] = image[(sy * img_w + sx) as usize];
        }
    }
    view
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
