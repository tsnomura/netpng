use clap::Parser;
use netpng_core::{flip, read_image, write_image, CompressionLevel, FlipMode};

/// Mirror or rotate an image by a multiple of 90 degrees (like pamflip).
#[derive(Parser)]
struct Args {
    /// Input path, "-" for stdin, or "clipboard".
    #[arg(long, default_value = "-")]
    input: String,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(long, default_value = "-")]
    output: String,
    #[arg(long, value_enum)]
    mode: FlipMode,
    #[arg(long, value_enum, default_value = "fast")]
    compression: CompressionLevel,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let img = read_image(&args.input)?;
    let out = flip(&img, args.mode);
    write_image(&args.output, &out, args.compression)
}
