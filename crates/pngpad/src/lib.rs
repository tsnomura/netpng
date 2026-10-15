use clap::Parser;
use netpng_core::{pad, parse_color, read_image, write_image, CompressionLevel};

/// Add a border of a solid color around an image (like pnmpad). The
/// opposite of pngcrop; original pixels (including their own
/// transparency) are copied in unchanged.
#[derive(Parser)]
struct Args {
    /// Input path, "-" or omitted for stdin, or "clipboard".
    #[arg(default_value = "-")]
    input: String,
    #[command(flatten)]
    common: netpng_core::CommonArgs,
    #[arg(long, value_enum, default_value = "fast")]
    compression: CompressionLevel,

    #[arg(long, default_value_t = 0)]
    top: u32,
    #[arg(long, default_value_t = 0)]
    bottom: u32,
    #[arg(long, default_value_t = 0)]
    left: u32,
    #[arg(long, default_value_t = 0)]
    right: u32,
    /// Border color as "R,G,B" or "R,G,B,A"; defaults to transparent.
    #[arg(long, default_value = "0,0,0,0")]
    color: String,
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
    anyhow::ensure!(
        args.top > 0 || args.bottom > 0 || args.left > 0 || args.right > 0,
        "at least one of --top/--bottom/--left/--right must be greater than 0"
    );
    let color = parse_color(&args.color)?;
    let img = read_image(&args.input)?;
    let out = pad(&img, args.top, args.bottom, args.left, args.right, color);
    write_image("-", &out, args.compression)
}
