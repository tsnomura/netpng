use clap::Parser;
use netpng_core::{blur, read_image, write_image, BlurChannels, CompressionLevel};

/// Apply a Gaussian blur. Use --channels alpha to feather a mask (e.g.
/// from pngchroma) without blurring its colors.
#[derive(Parser)]
struct Args {
    /// Input path, "-" or omitted for stdin, or "clipboard".
    #[arg(default_value = "-")]
    input: String,
    #[command(flatten)]
    common: netpng_core::CommonArgs,
    /// Gaussian sigma (blur strength) in pixels.
    #[arg(long)]
    sigma: f64,
    #[arg(long, value_enum, default_value = "all")]
    channels: BlurChannels,
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
    let img = read_image(&args.input)?;
    let out = blur(&img, args.sigma, args.channels)?;
    write_image("-", &out, args.compression)
}
