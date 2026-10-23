use clap::Parser;
use netpng_core::{blur, read_image, write_image, BlurChannels, CompressionLevel};

/// Apply a Gaussian blur. Use --channels alpha to feather a mask (e.g.
/// from pngchroma) without blurring its colors.
#[derive(Parser)]
#[command(version)]
struct Args {
    #[command(flatten)]
    input: netpng_core::InputArgs,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(short = 'o', long, default_value = "-")]
    output: String,
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
    let args = Args::parse_from(args);
    let input = args.input.path();
    let img = read_image(input)?;
    let out = blur(&img, args.sigma, args.channels)?;
    write_image(&args.output, &out, args.compression)
}
