use clap::Parser;
use netpng_core::{invert, read_image, write_image, CompressionLevel};

/// Invert the RGB channels of an image (like pnminvert). Alpha is untouched.
#[derive(Parser)]
struct Args {
    /// Input path, "-" for stdin, or "clipboard".
    #[arg(long, default_value = "-")]
    input: String,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(long, default_value = "-")]
    output: String,
    #[arg(long, value_enum, default_value = "fast")]
    compression: CompressionLevel,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let img = read_image(&args.input)?;
    let out = invert(&img);
    write_image(&args.output, &out, args.compression)
}
