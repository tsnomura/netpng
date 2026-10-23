use clap::Parser;
use netpng_core::{composite_over, read_image, write_image, CompressionLevel};

/// Alpha-composite an overlay image onto a base image (Porter-Duff "over").
/// The output has the base image's dimensions; the overlay is clipped
/// silently wherever it falls outside those bounds.
#[derive(Parser)]
struct Args {
    /// Base (bottom) image path, "-" for stdin, or "clipboard".
    #[arg(value_name = "BASE")]
    base: Option<String>,
    /// Overlay (top) image path, "-" for stdin, or "clipboard".
    #[arg(value_name = "OVERLAY")]
    overlay: Option<String>,
    /// Base (bottom) image; takes precedence over the positional BASE.
    #[arg(long = "base", value_name = "BASE")]
    base_flag: Option<String>,
    /// Overlay (top) image; takes precedence over the positional OVERLAY.
    #[arg(long = "overlay", value_name = "OVERLAY")]
    overlay_flag: Option<String>,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(long, default_value = "-")]
    output: String,
    #[command(flatten)]
    common: netpng_core::CommonArgs,
    /// X offset of the overlay's top-left corner on the base canvas.
    #[arg(long, allow_hyphen_values = true, default_value_t = 0)]
    x: i32,
    /// Y offset of the overlay's top-left corner on the base canvas.
    #[arg(long, allow_hyphen_values = true, default_value_t = 0)]
    y: i32,
    #[arg(long, value_enum, default_value = "fast")]
    compression: CompressionLevel,
}

pub fn run<I, T>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let argv = netpng_core::normalize_args(args);
    if netpng_core::print_version_if_requested(&argv, env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")) {
        return Ok(());
    }
    let args = Args::parse_from(argv);
    let _ = args.common.quiet;
    let base = args
        .base_flag
        .or(args.base)
        .ok_or_else(|| anyhow::anyhow!("a base image is required (positional BASE or --base)"))?;
    let overlay = args
        .overlay_flag
        .or(args.overlay)
        .ok_or_else(|| anyhow::anyhow!("an overlay image is required (positional OVERLAY or --overlay)"))?;
    anyhow::ensure!(
        !(base == "-" && overlay == "-"),
        "base and overlay can't both be \"-\" (stdin can only be read once)"
    );

    let base = read_image(&base)?;
    let overlay = read_image(&overlay)?;
    let result = composite_over(&base, &overlay, args.x, args.y);
    write_image(&args.output, &result, args.compression)
}
