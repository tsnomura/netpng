use clap::Parser;
use netpng_core::{dist_grayscale, read_image, write_image, CompressionLevel, DistSortOrder};

/// Convert a low-color-count image to grayscale, spacing gray levels to
/// maximize contrast between the original colors (like ppmdist).
#[derive(Parser)]
struct Args {
    /// Input path, "-" for stdin, or "clipboard".
    #[arg(long, default_value = "-")]
    input: String,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(long, default_value = "-")]
    output: String,
    #[arg(long, value_enum, default_value = "intensity")]
    order: DistSortOrder,
    #[arg(long, value_enum, default_value = "fast")]
    compression: CompressionLevel,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let img = read_image(&args.input)?;
    let out = dist_grayscale(&img, args.order);
    write_image(&args.output, &out, args.compression)
}
