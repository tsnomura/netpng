use clap::Parser;
use netpng_core::{blur, read_image, write_image, BlurChannels, CompressionLevel};

/// Apply a Gaussian blur. Use --channels alpha to feather a mask (e.g.
/// from pngchroma) without blurring its colors.
#[derive(Parser)]
struct Args {
    /// Input path, "-" for stdin, or "clipboard".
    #[arg(long, default_value = "-")]
    input: String,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(long, default_value = "-")]
    output: String,
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
    let args = Args::parse_from(args);
    let img = read_image(&args.input)?;
    let out = blur(&img, args.sigma, args.channels)?;
    write_image(&args.output, &out, args.compression)
}
