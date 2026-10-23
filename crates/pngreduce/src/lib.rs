use clap::Parser;
use netpng_core::{quantize, read_image, write_image, CompressionLevel};

/// Reduce an image to at most --colors distinct RGB colors via median-cut
/// quantization (like pnmquant). Alpha is untouched.
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
    #[arg(long)]
    colors: u32,
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
    let out = quantize(&img, args.colors)?;
    write_image(&args.output, &out, args.compression)
}
