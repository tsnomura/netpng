use clap::Parser;
use netpng_core::{flip, read_image, write_image, CompressionLevel, FlipMode};

/// Mirror or rotate an image by a multiple of 90 degrees (like pamflip).
#[derive(Parser)]
struct Args {
    /// Input path, "-" or omitted for stdin, or "clipboard".
    #[arg(default_value = "-")]
    input: String,
    #[command(flatten)]
    common: netpng_core::CommonArgs,
    #[arg(long, value_enum)]
    mode: FlipMode,
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
    let out = flip(&img, args.mode);
    write_image("-", &out, args.compression)
}
