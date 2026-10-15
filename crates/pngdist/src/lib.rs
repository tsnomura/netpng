use clap::Parser;
use netpng_core::{dist_grayscale, read_image, write_image, CompressionLevel, DistSortOrder};

/// Convert a low-color-count image to grayscale, spacing gray levels to
/// maximize contrast between the original colors (like ppmdist).
#[derive(Parser)]
struct Args {
    /// Input path, "-" or omitted for stdin, or "clipboard".
    #[arg(default_value = "-")]
    input: String,
    #[command(flatten)]
    common: netpng_core::CommonArgs,
    #[arg(long, value_enum, default_value = "intensity")]
    order: DistSortOrder,
    #[arg(long, value_enum, default_value = "fast")]
    compression: CompressionLevel,
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
    let _ = args.common.quiet;
    let img = read_image(&args.input)?;
    let out = dist_grayscale(&img, args.order);
    write_image("-", &out, args.compression)
}
