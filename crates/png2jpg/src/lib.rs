use clap::Parser;
use netpng_core::{parse_color, read_image, write_jpeg};

/// Convert a PNG image to baseline JPEG (like pnmtojpeg). JPEG has no
/// alpha channel, so the image is first flattened onto --background
/// (default: white).
#[derive(Parser)]
#[command(version)]
struct Args {
    #[command(flatten)]
    input: netpng_core::InputArgs,
    /// Output path, "-" for stdout.
    #[arg(short = 'o', long, default_value = "-")]
    output: String,
    #[command(flatten)]
    common: netpng_core::CommonArgs,
    /// Background color as "R,G,B" to flatten alpha onto.
    #[arg(long, default_value = "255,255,255")]
    background: String,
    /// JPEG quality, 1 (worst) to 100 (best).
    #[arg(long, default_value_t = 90)]
    quality: u8,
}

pub fn run<I, T>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let args = Args::parse_from(args);
    anyhow::ensure!((1..=100).contains(&args.quality), "--quality must be 1-100");

    let input = args.input.path();
    let img = read_image(input)?;
    let [r, g, b, _] = parse_color(&args.background)?;
    write_jpeg(&args.output, &img, [r, g, b], args.quality)
}
