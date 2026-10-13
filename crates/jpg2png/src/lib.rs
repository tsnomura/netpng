use clap::Parser;
use netpng_core::{read_jpeg, write_image, CompressionLevel};

/// Convert a JPEG image to PNG (like jpegtopnm). The JPEG is already
/// lossy; this conversion itself is lossless.
#[derive(Parser)]
struct Args {
    /// Input path, "-" for stdin.
    #[arg(long, default_value = "-")]
    input: String,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(long, default_value = "-")]
    output: String,
    #[arg(long, value_enum, default_value = "fast")]
    compression: CompressionLevel,
}

pub fn run<I, T>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let args = Args::parse_from(args);
    let img = read_jpeg(&args.input)?;
    write_image(&args.output, &img, args.compression)
}
