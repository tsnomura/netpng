use clap::Parser;
use netpng_core::{decontaminate, parse_color, read_image, write_image, CompressionLevel};

/// Remove known-background color spill from an already alpha-keyed image
/// (e.g. the output of pngchroma), reconstructing each partially
/// transparent pixel's true foreground color given the same background
/// color used to key it. Fully opaque/transparent pixels are untouched.
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
    /// The background color that was keyed out, as "R,G,B".
    #[arg(long)]
    background: String,
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
    let out = decontaminate(&img, [r, g, b]);
    write_image(&args.output, &out, args.compression)
}
