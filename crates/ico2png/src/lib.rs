use clap::Parser;
use netpng_core::{read_ico, write_image, CompressionLevel};

/// Extract one image from an ICO file as PNG. An ICO can hold several
/// sizes of the same image; by default the largest is used, or the one
/// whose width is closest to --size.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Input path (ICO), "-" or omitted for stdin.
    #[arg(value_name = "INPUT", conflicts_with = "input_flag")]
    input: Option<String>,
    /// Same as INPUT, as a flag (give one or the other, not both).
    #[arg(short = 'i', long = "input", value_name = "INPUT")]
    input_flag: Option<String>,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(short = 'o', long, default_value = "-")]
    output: String,
    #[command(flatten)]
    common: netpng_core::CommonArgs,
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
    let input = args
        .input_flag
        .as_deref()
        .or(args.input.as_deref())
        .unwrap_or("-");
    let img = read_ico(input, args.size)?;
    write_image(&args.output, &img, args.compression)
}
