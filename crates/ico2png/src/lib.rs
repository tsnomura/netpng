use clap::Parser;
use netpng_core::{read_ico, write_image, CompressionLevel};

/// Extract one image from an ICO file as PNG. An ICO can hold several
/// sizes of the same image; by default the largest is used, or the one
/// whose width is closest to --size.
#[derive(Parser)]
struct Args {
    /// Input path (ICO), "-" or omitted for stdin.
    #[arg(default_value = "-")]
    input: String,
    /// Print the version and exit.
    #[arg(short = 'v', long)]
    version: bool,
    /// Suppress informational messages (errors are still printed).
    #[arg(short = 'q', long)]
    quiet: bool,
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
    let args = Args::parse_from(netpng_core::normalize_args(args));
    if args.version {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let _ = args.quiet;
    let img = read_ico(&args.input, args.size)?;
    write_image("-", &img, args.compression)
}
