use clap::Parser;
use netpng_core::{read_image, write_image, CompressionLevel, Image};
use softbuffer::{Context, Surface};
use std::num::NonZeroU32;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

/// Display an image in a window (reads from a pipe, a file, or the
/// clipboard). Transparent areas are shown over a checkerboard.
///
/// Controls: mouse wheel zooms (centered on the cursor); left-click-drag
/// pans; Space cycles zoom between --zoom, actual size (100%), and fit-to-
/// window; Enter re-centers the image; "0" resets zoom/pan; "s" saves the
/// image to a timestamped PNG in the current directory; "c" copies it to
/// the clipboard; the window is resizable; Esc or "q" or closing the
/// window exits.
///
/// "c" (or --copy-to-clipboard) is the practical way to get a result onto
/// the clipboard on Linux: X11's clipboard only serves content for as long
/// as the setting process stays alive, so a one-shot CLI call that writes
/// the clipboard loses it the instant it exits. pngview is already
/// long-lived (it runs until you close it, or --timeout elapses), so
/// copying from here keeps working for as long as the window stays open.
#[derive(Parser)]
struct Args {
    /// Input path, "-" or omitted for stdin, or "clipboard".
    #[arg(default_value = "-")]
    input: String,
    #[command(flatten)]
    common: netpng_core::CommonArgs,
    /// Zoom level Space cycles to as its first step (2.0 = 200%).
    #[arg(long, default_value_t = 2.0)]
    zoom: f64,
    /// Copy the image to the clipboard on startup, as if "c" were pressed
    /// immediately. Handy for scripting: e.g. pipe a result in with this
    /// and --timeout so it flashes on screen, lands on the clipboard, and
    /// closes itself.
    #[arg(long)]
    copy_to_clipboard: bool,
    /// Close the window automatically after this many seconds.
    #[arg(long)]
    timeout: Option<f64>,
}

const MIN_ZOOM: f64 = 0.05;
const MAX_ZOOM: f64 = 32.0;
/// Zoom multiplier per unit of scroll-wheel delta; smaller = gentler.
/// winit reports one wheel notch as a delta of 1.0 on every platform, so
/// this is the per-notch zoom step.
const ZOOM_WHEEL_FACTOR: f64 = 1.1;
/// Small images (icons, sprites, our own test fixtures...) would otherwise
/// open in a window as tiny as the image itself; scale up so the window's
/// larger side is at least this many pixels.
const MIN_INITIAL_WINDOW: f64 = 400.0;
/// Color for area outside the image canvas (panned/zoomed past its edge),
/// distinct from the checkerboard used for transparent pixels *within* it.
const OUTSIDE_CANVAS: u32 = 0x00303030;

/// One-shot actions triggered by a key press.
#[derive(Debug)]
enum Action {
    Quit,
    Save,
    Copy,
    CycleZoom,
    Center,
    Reset,
}

/// Test hook: log every key event and focus change to stderr.
fn pv_debug() -> bool {
    std::env::var_os("PV_DEBUG").is_some()
}

/// Map a key to a pngview action, or None for keys we don't act on.
/// Letter keys are matched case-insensitively so they still work with
/// Shift held (or on layouts where the shifted symbol differs).
fn key_action(key: &Key) -> Option<Action> {
    match key {
        Key::Named(NamedKey::Escape) => Some(Action::Quit),
        Key::Named(NamedKey::Space) => Some(Action::CycleZoom),
        // Numpad Enter arrives as NamedKey::Enter on X11 (KP_Enter);
        // Numpad 0 with NumLock on arrives as the character "0".
        Key::Named(NamedKey::Enter) => Some(Action::Center),
        Key::Character(c) => match c.as_str() {
            "q" | "Q" => Some(Action::Quit),
            "s" | "S" => Some(Action::Save),
            "c" | "C" => Some(Action::Copy),
            "0" => Some(Action::Reset),
            _ => None,
        },
        _ => None,
    }
}

/// The application state, driven by winit's event loop. Input arrives as
/// events (a key press is an event, not a polled state), so a quick tap
/// can never be missed between frames, and auto-repeat is ignored via the
/// event's `repeat` flag. The frame is only re-rendered and re-presented
/// when something changed, so an idle window costs no CPU.
struct App {
    /// Owns the window (softbuffer keeps it alive) and the software
    /// framebuffer presented into it.
    surface: Option<Surface<OwnedDisplayHandle, Window>>,
    image: Image,
    checkerboarded: Vec<u32>,
    base_zoom: f64,
    win_w: u32,
    win_h: u32,
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
    last_cursor: Option<PhysicalPosition<f64>>,
    left_down: bool,
    zoom_cycle: u8,
    deadline: Option<Instant>,
}

impl ApplicationHandler for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::Init) {
            // Block on events instead of spinning; with --timeout, wake at
            // the deadline and exit in about_to_wait.
            event_loop.set_control_flow(match self.deadline {
                Some(deadline) => ControlFlow::WaitUntil(deadline),
                None => ControlFlow::Wait,
            });
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Redundant Resumed events are possible; only initialize once.
        if self.surface.is_some() {
            self.draw();
            return;
        }
        let context = match Context::new(event_loop.owned_display_handle()) {
            Ok(context) => context,
            Err(e) => {
                eprintln!("pngview: failed to initialize the drawing surface: {e}");
                event_loop.exit();
                return;
            }
        };
        let attrs = WindowAttributes::default()
            .with_title("pngview")
            .with_inner_size(PhysicalSize::new(self.win_w, self.win_h))
            .with_resizable(true);
        // WSLg's Wayland compositor closes the connection when
        // smithay-client-toolkit's client-side decorations (sctk-adwaita) try
        // to set up their border subsurfaces -- disable decorations on Linux
        // to avoid that code path. Windows/macOS keep native decorations.
        #[cfg(target_os = "linux")]
        let attrs = attrs.with_decorations(false);
        let window = match event_loop.create_window(attrs) {
            Ok(window) => window,
            Err(e) => {
                eprintln!("pngview: failed to create the window: {e}");
                event_loop.exit();
                return;
            }
        };
        match Surface::new(&context, window) {
            Ok(surface) => self.surface = Some(surface),
            Err(e) => {
                eprintln!("pngview: failed to initialize the drawing surface: {e}");
                event_loop.exit();
                return;
            }
        }
        self.request_draw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::RedrawRequested => self.draw(),

            WindowEvent::Resized(size) => {
                if let Some(surface) = self.surface.as_mut() {
                    if size.width > 0 && size.height > 0 {
                        if let (Some(w), Some(h)) =
                            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                        {
                            if let Err(e) = surface.resize(w, h) {
                                eprintln!("pngview: failed to resize the surface: {e}");
                            }
                        }
                    }
                }
                self.request_draw();
            }

            WindowEvent::KeyboardInput {
                event: key_event,
                is_synthetic,
                ..
            } => {
                // Only act on real first presses: ignore OS auto-repeat
                // (holding a key must not retrigger a one-shot action) and
                // the synthetic presses winit generates when a window
                // regains focus with keys still held.
                if is_synthetic || key_event.repeat || key_event.state != ElementState::Pressed {
                    if pv_debug() {
                        eprintln!(
                            "pv-debug: key {:?} ignored (state={:?}, repeat={}, synthetic={})",
                            key_event.logical_key, key_event.state, key_event.repeat,
                            is_synthetic
                        );
                    }
                    return;
                }
                let Some(action) = key_action(&key_event.logical_key) else {
                    if pv_debug() {
                        eprintln!(
                            "pv-debug: key {:?} pressed (no action)",
                            key_event.logical_key
                        );
                    }
                    return;
                };
                if pv_debug() {
                    eprintln!(
                        "pv-debug: key {:?} pressed -> {:?}",
                        key_event.logical_key, action
                    );
                }
                match action {
                    Action::Quit => event_loop.exit(),
                    Action::Save => {
                        let path = next_save_path();
                        match write_image(
                            &path.to_string_lossy(),
                            &self.image,
                            CompressionLevel::Default,
                        ) {
                            Ok(()) => eprintln!("pngview: saved to {}", path.display()),
                            Err(e) => eprintln!("pngview: failed to save: {e}"),
                        }
                    }
                    Action::Copy => match write_image(
                        "clipboard",
                        &self.image,
                        CompressionLevel::Default,
                    ) {
                        Ok(()) => eprintln!("pngview: copied to clipboard"),
                        Err(e) => eprintln!("pngview: failed to copy to clipboard: {e}"),
                    },
                    Action::CycleZoom => {
                        let (win_w, win_h) = self.win_size();
                        self.zoom = match self.zoom_cycle {
                            0 => self.base_zoom,
                            1 => 1.0,
                            _ => fit_zoom(self.image.width, self.image.height, win_w, win_h),
                        }
                        .clamp(MIN_ZOOM, MAX_ZOOM);
                        self.zoom_cycle = (self.zoom_cycle + 1) % 3;
                        (self.pan_x, self.pan_y) = centered_pan(
                            self.image.width,
                            self.image.height,
                            win_w,
                            win_h,
                            self.zoom,
                        );
                        self.request_draw();
                    }
                    Action::Center => {
                        let (win_w, win_h) = self.win_size();
                        (self.pan_x, self.pan_y) = centered_pan(
                            self.image.width,
                            self.image.height,
                            win_w,
                            win_h,
                            self.zoom,
                        );
                        self.request_draw();
                    }
                    Action::Reset => {
                        self.zoom = 1.0;
                        self.pan_x = 0.0;
                        self.pan_y = 0.0;
                        self.zoom_cycle = 0;
                        self.request_draw();
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                if self.left_down {
                    if let Some(last) = self.last_cursor {
                        // Keep the image point under the cursor fixed.
                        self.pan_x -= (position.x - last.x) / self.zoom;
                        self.pan_y -= (position.y - last.y) / self.zoom;
                    }
                }
                self.last_cursor = Some(position);
                if self.left_down {
                    self.request_draw();
                }
            }

            WindowEvent::MouseInput { button, state, .. } => {
                if button == MouseButton::Left {
                    self.left_down = state == ElementState::Pressed;
                    // Restart pan tracking so the first move after a press
                    // doesn't apply a stale delta.
                    if self.left_down {
                        self.last_cursor = None;
                    }
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                // Normalize pixel-based deltas (Windows, some mice) to
                // line-based ones (~100px per notch) so one notch zooms by
                // the same factor regardless of platform.
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(pos) => pos.y / 100.0,
                };
                if dy.abs() > f64::EPSILON {
                    let old_zoom = self.zoom;
                    self.zoom = (self.zoom * ZOOM_WHEEL_FACTOR.powf(dy)).clamp(MIN_ZOOM, MAX_ZOOM);
                    if let Some(cursor) = self.last_cursor {
                        // Keep the image point under the cursor fixed.
                        let anchor_x = self.pan_x + cursor.x / old_zoom;
                        let anchor_y = self.pan_y + cursor.y / old_zoom;
                        self.pan_x = anchor_x - cursor.x / self.zoom;
                        self.pan_y = anchor_y - cursor.y / self.zoom;
                    }
                    self.request_draw();
                }
            }

            WindowEvent::CloseRequested | WindowEvent::Destroyed => event_loop.exit(),

            WindowEvent::Focused(has_focus) => {
                if pv_debug() {
                    eprintln!("pv-debug: Focused({has_focus})");
                }
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // ControlFlow::WaitUntil only wakes the loop once, at the deadline;
        // exit from here.
        if self.deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            event_loop.exit();
        }
    }
}

impl App {
    /// Current window client size in physical pixels (never zero).
    fn win_size(&self) -> (usize, usize) {
        self.surface
            .as_ref()
            .map(|surface| surface.window().inner_size())
            .map(|s| (s.width.max(1) as usize, s.height.max(1) as usize))
            .unwrap_or((1, 1))
    }

    fn request_draw(&self) {
        if let Some(surface) = &self.surface {
            surface.window().request_redraw();
        }
    }

    /// Render the current view into the framebuffer and present it.
    fn draw(&mut self) {
        let Some(size) = self
            .surface
            .as_ref()
            .map(|surface| surface.window().inner_size())
        else {
            return;
        };
        let (w, h) = (size.width, size.height);
        if w == 0 || h == 0 {
            return;
        }
        // Build the frame (32-bit 0x00RRGGBB, the softbuffer/X11 pixel
        // format) before borrowing the surface mutably.
        let view = render_view(
            &self.checkerboarded,
            self.image.width,
            self.image.height,
            w as usize,
            h as usize,
            self.zoom,
            self.pan_x,
            self.pan_y,
        );
        let Some(surface) = self.surface.as_mut() else {
            return;
        };
        // Surface::new leaves the buffer unsized and present() panics on a
        // zero-sized buffer, so make sure the buffer matches the window
        // (resize is a no-op when the size is unchanged).
        if w > 0 && h > 0 {
            if let (Some(bw), Some(bh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) {
                if let Err(e) = surface.resize(bw, bh) {
                    eprintln!("pngview: failed to resize the surface: {e}");
                    return;
                }
            }
        }
        let mut buffer = match surface.buffer_mut() {
            Ok(buffer) => buffer,
            Err(e) => {
                eprintln!("pngview: failed to acquire the frame buffer: {e}");
                return;
            }
        };
        let pixels = &mut *buffer;
        if pixels.len() == view.len() {
            pixels.copy_from_slice(&view);
        }
        if let Err(e) = buffer.present() {
            eprintln!("pngview: failed to present the frame: {e}");
        }
    }
}

pub fn run<I, T>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let args = Args::parse_from(netpng_core::normalize_args(args));
    if args.common.print_version_and_exit(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")) {
        return Ok(());
    }
    let img = read_image(&args.input)?;
    let checkerboarded = composite_checkerboard(&img);

    if !args.common.quiet {
        eprintln!(
            "pngview: wheel = zoom, left-drag = pan, space = cycle zoom, enter = center, \
             0 = reset, s = save, c = copy to clipboard, Esc/q = quit"
        );
    }

    let larger_side = (img.width.max(img.height) as f64).max(1.0);
    let initial_zoom = (MIN_INITIAL_WINDOW / larger_side).max(1.0);
    let initial_win_w = ((img.width as f64) * initial_zoom).round() as u32;
    let initial_win_h = ((img.height as f64) * initial_zoom).round() as u32;

    if args.copy_to_clipboard {
        match write_image("clipboard", &img, CompressionLevel::Default) {
            Ok(()) => eprintln!("pngview: copied to clipboard"),
            Err(e) => eprintln!("pngview: failed to copy to clipboard: {e}"),
        }
    }

    let start = Instant::now();
    let deadline = args.timeout.map(|t| start + Duration::from_secs_f64(t));

    let event_loop = EventLoop::new()
        .map_err(|e| anyhow::anyhow!("failed to create the event loop: {e}"))?;
    let mut app = App {
        surface: None,
        image: img,
        checkerboarded,
        base_zoom: args.zoom,
        win_w: initial_win_w,
        win_h: initial_win_h,
        zoom: initial_zoom,
        pan_x: 0.0,
        pan_y: 0.0,
        last_cursor: None,
        left_down: false,
        zoom_cycle: 0,
        deadline,
    };
    event_loop
        .run_app(&mut app)
        .map_err(|e| anyhow::anyhow!("event loop error: {e}"))?;
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
