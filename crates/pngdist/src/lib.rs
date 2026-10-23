use clap::Parser;
use netpng_core::{dist_grayscale, read_image, write_image, CompressionLevel, DistSortOrder};

/// Convert a low-color-count image to grayscale, spacing gray levels to
/// maximize contrast between the original colors (like ppmdist).
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
    #[arg(long, value_enum, default_value = "intensity")]
    order: DistSortOrder,
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
    let out = dist_grayscale(&img, args.order);
    write_image(&args.output, &out, args.compression)
}
