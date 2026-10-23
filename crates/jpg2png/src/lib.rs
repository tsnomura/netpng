use clap::Parser;
use netpng_core::{read_jpeg, write_image, CompressionLevel};

/// Convert a JPEG image to PNG (like jpegtopnm). The JPEG is already
/// lossy; this conversion itself is lossless.
#[derive(Parser)]
struct Args {
    /// Input path (JPEG), "-" or omitted for stdin.
    #[arg(default_value = "-", value_name = "INPUT")]
    input: String,
    /// Same as INPUT, as a flag; takes precedence over the positional
    /// argument when both are given.
    #[arg(long = "input", value_name = "INPUT")]
    input_flag: Option<String>,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(long, default_value = "-")]
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
    let argv = netpng_core::normalize_args(args);
    if netpng_core::print_version_if_requested(&argv, env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")) {
        return Ok(());
    }
    let args = Args::parse_from(argv);
    let _ = args.common.quiet;
    let input = args.input_flag.as_deref().unwrap_or(&args.input);
    let img = read_jpeg(input)?;
    write_image(&args.output, &img, args.compression)
}
