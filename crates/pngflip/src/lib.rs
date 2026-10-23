use clap::Parser;
use netpng_core::{flip, read_image, write_image, CompressionLevel, FlipMode};

/// Mirror or rotate an image by a multiple of 90 degrees (like pamflip).
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
    #[arg(long, value_enum)]
    mode: FlipMode,
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
    let out = flip(&img, args.mode);
    write_image(&args.output, &out, args.compression)
}
