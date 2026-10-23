use clap::Parser;
use netpng_core::{read_ico, write_image, CompressionLevel};

/// Extract one image from an ICO file as PNG. An ICO can hold several
/// sizes of the same image; by default the largest is used, or the one
/// whose width is closest to --size.
#[derive(Parser)]
struct Args {
    /// Input path (ICO), "-" or omitted for stdin.
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
    let argv = netpng_core::normalize_args(args);
    if netpng_core::print_version_if_requested(&argv, env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")) {
        return Ok(());
    }
    let args = Args::parse_from(argv);
    let _ = args.common.quiet;
    let input = args.input_flag.as_deref().unwrap_or(&args.input);
    let img = read_ico(input, args.size)?;
    write_image(&args.output, &img, args.compression)
}
