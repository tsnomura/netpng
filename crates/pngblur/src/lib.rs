use clap::Parser;
use netpng_core::{blur, read_image, write_image, BlurChannels, CompressionLevel};

/// Apply a Gaussian blur. Use --channels alpha to feather a mask (e.g.
/// from pngchroma) without blurring its colors.
#[derive(Parser)]
struct Args {
    /// Input path, "-" or omitted for stdin, or "clipboard".
    #[arg(default_value = "-")]
    input: String,
    /// Print the version and exit.
    #[arg(short = 'v', long)]
    version: bool,
    /// Suppress informational messages (errors are still printed).
    #[arg(short = 'q', long)]
    quiet: bool,
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
    let args = Args::parse_from(netpng_core::normalize_args(args));
    if args.version {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let _ = args.quiet;
    let img = read_image(&args.input)?;
    let out = blur(&img, args.sigma, args.channels)?;
    write_image("-", &out, args.compression)
}
