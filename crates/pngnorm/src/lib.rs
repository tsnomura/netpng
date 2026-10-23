use clap::Parser;
use netpng_core::{normalize, read_image, write_image, CompressionLevel};

/// Stretch each of R, G, B independently to use the full 0-255 range,
/// clipping a percentage of the darkest/brightest pixels (like pnmnorm's
/// default per-channel mode).
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
    /// Percentage of the darkest pixel values to clip to black.
    #[arg(long, default_value_t = 2.0)]
    bpercent: f64,
    /// Percentage of the brightest pixel values to clip to white.
    #[arg(long, default_value_t = 1.0)]
    wpercent: f64,
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
    let out = normalize(&img, args.bpercent, args.wpercent);
    write_image(&args.output, &out, args.compression)
}
