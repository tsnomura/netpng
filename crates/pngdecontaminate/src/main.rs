use clap::Parser;
use netpng_core::{decontaminate, parse_color, read_image, write_image, CompressionLevel};

/// Remove known-background color spill from an already alpha-keyed image
/// (e.g. the output of pngchroma), reconstructing each partially
/// transparent pixel's true foreground color given the same background
/// color used to key it. Fully opaque/transparent pixels are untouched.
#[derive(Parser)]
struct Args {
    /// Input path, "-" for stdin, or "clipboard".
    #[arg(long, default_value = "-")]
    input: String,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(long, default_value = "-")]
    output: String,
    /// The background color that was keyed out, as "R,G,B".
    #[arg(long)]
    background: String,
    #[arg(long, value_enum, default_value = "fast")]
    compression: CompressionLevel,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let [r, g, b, _] = parse_color(&args.background)?;
    let img = read_image(&args.input)?;
    let out = decontaminate(&img, [r, g, b]);
    write_image(&args.output, &out, args.compression)
}
