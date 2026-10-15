use clap::Parser;
use netpng_core::{read_jpeg, write_image, CompressionLevel};

/// Convert a JPEG image to PNG (like jpegtopnm). The JPEG is already
/// lossy; this conversion itself is lossless.
#[derive(Parser)]
struct Args {
    /// Input path (JPEG), "-" or omitted for stdin.
    #[arg(default_value = "-")]
    input: String,
    #[command(flatten)]
    common: netpng_core::CommonArgs,
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
    let img = read_jpeg(&args.input)?;
    write_image("-", &img, args.compression)
}
