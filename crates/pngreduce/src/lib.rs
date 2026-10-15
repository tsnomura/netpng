use clap::Parser;
use netpng_core::{quantize, read_image, write_image, CompressionLevel};

/// Reduce an image to at most --colors distinct RGB colors via median-cut
/// quantization (like pnmquant). Alpha is untouched.
#[derive(Parser)]
struct Args {
    /// Input path, "-" or omitted for stdin, or "clipboard".
    #[arg(default_value = "-")]
    input: String,
    /// Print the version and exit.
    #[arg(short = 'v', long)]
    version: bool,
    /// Suppress informational messages (errors are still printed).
    #[arg(short = 'q', long)]
    quiet: bool,
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
    let args = Args::parse_from(netpng_core::normalize_args(args));
    if args.version {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let _ = args.quiet;
    let img = read_image(&args.input)?;
    let out = quantize(&img, args.colors)?;
    write_image("-", &out, args.compression)
}
