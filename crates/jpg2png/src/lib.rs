use clap::Parser;
use netpng_core::{read_jpeg, write_image, CompressionLevel};

/// Convert a JPEG image to PNG (like jpegtopnm). The JPEG is already
/// lossy; this conversion itself is lossless.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Input path (JPEG), "-" or omitted for stdin.
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
    let img = read_jpeg(input)?;
    write_image(&args.output, &img, args.compression)
}
