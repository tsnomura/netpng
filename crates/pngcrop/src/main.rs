use clap::Parser;
use netpng_core::{read_image, write_image, CompressionLevel};

/// Crop a PNG image to a rectangle.
#[derive(Parser)]
struct Args {
    /// Input path, "-" for stdin, or "clipboard".
    #[arg(long, default_value = "-")]
    input: String,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(long, default_value = "-")]
    output: String,
    #[arg(long)]
    x: u32,
    #[arg(long)]
    y: u32,
    #[arg(long)]
    width: u32,
    #[arg(long)]
    height: u32,
    #[arg(long, value_enum, default_value = "fast")]
    compression: CompressionLevel,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let img = read_image(&args.input)?;
    let cropped = netpng_core::crop(&img, args.x, args.y, args.width, args.height)?;
    write_image(&args.output, &cropped, args.compression)
}
