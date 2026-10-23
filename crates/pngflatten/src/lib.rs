use clap::Parser;
use netpng_core::{flatten, parse_color, read_image, write_image, CompressionLevel};

/// Flatten transparency onto a solid background color (like pnmflatten),
/// producing a fully-opaque PNG. Unlike png2jpg, the output stays PNG.
#[derive(Parser)]
#[command(version)]
struct Args {
    #[command(flatten)]
    input: netpng_core::InputArgs,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(short = 'o', long, default_value = "-")]
    output: String,
    /// Background color as "R,G,B" to flatten alpha onto.
    #[arg(long, default_value = "255,255,255")]
    background: String,
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
    let args = Args::parse_from(args);
    let [r, g, b, _] = parse_color(&args.background)?;
    let input = args.input.path();
    let img = read_image(input)?;
    let out = flatten(&img, [r, g, b]);
    write_image(&args.output, &out, args.compression)
}
