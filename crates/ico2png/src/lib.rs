use clap::Parser;
use netpng_core::{read_ico, write_image, CompressionLevel};

/// Extract one image from an ICO file as PNG. An ICO can hold several
/// sizes of the same image; by default the largest is used, or the one
/// whose width is closest to --size.
#[derive(Parser)]
struct Args {
    /// Input path, "-" for stdin.
    #[arg(long, default_value = "-")]
    input: String,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(long, default_value = "-")]
    output: String,
    /// Pick the entry whose width is closest to this (default: largest).
    #[arg(long)]
    size: Option<u32>,
    #[arg(long, value_enum, default_value = "fast")]
    compression: CompressionLevel,
}

pub fn run<I, T>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let args = Args::parse_from(args);
    let img = read_ico(&args.input, args.size)?;
    write_image(&args.output, &img, args.compression)
}
