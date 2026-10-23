use clap::Parser;
use netpng_core::{composite_over, read_image, write_image, CompressionLevel};

/// Alpha-composite an overlay image onto a base image (Porter-Duff "over").
/// The output has the base image's dimensions; the overlay is clipped
/// silently wherever it falls outside those bounds.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Base (bottom) image path, "-" for stdin, or "clipboard".
    #[arg(value_name = "BASE")]
    base: Option<String>,
    /// Overlay (top) image path, "-" for stdin, or "clipboard".
    #[arg(value_name = "OVERLAY")]
    overlay: Option<String>,
    /// Base (bottom) image, as a flag; positional images then fill the
    /// remaining slot(s) in BASE, OVERLAY order.
    #[arg(long = "base", value_name = "BASE")]
    base_flag: Option<String>,
    /// Overlay (top) image, as a flag; positional images then fill the
    /// remaining slot(s) in BASE, OVERLAY order.
    #[arg(long = "overlay", value_name = "OVERLAY")]
    overlay_flag: Option<String>,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(short = 'o', long, default_value = "-")]
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
    let args = Args::parse_from(args);
    // Flags claim their slot first; positionals fill whatever is left, in
    // order, so `--base bg.png fg.png` and `--overlay fg.png bg.png` both work.
    let mut positionals = args.base.into_iter().chain(args.overlay);
    let base = args
        .base_flag
        .or_else(|| positionals.next())
        .ok_or_else(|| anyhow::anyhow!("a base image is required (positional BASE or --base)"))?;
    let overlay = args
        .overlay_flag
        .or_else(|| positionals.next())
        .ok_or_else(|| anyhow::anyhow!("an overlay image is required (positional OVERLAY or --overlay)"))?;
    anyhow::ensure!(
        positionals.next().is_none(),
        "too many images: give BASE and OVERLAY once each (positionally or via --base/--overlay)"
    );
    anyhow::ensure!(
        !(base == "-" && overlay == "-"),
        "base and overlay can't both be \"-\" (stdin can only be read once)"
    );

    let base = read_image(&base)?;
    let overlay = read_image(&overlay)?;
    let result = composite_over(&base, &overlay, args.x, args.y);
    write_image(&args.output, &result, args.compression)
}
