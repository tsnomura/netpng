use clap::Parser;
use netpng_core::{read_image, write_image, CompressionLevel};

/// Copy the PNG image on the clipboard to stdout or a file, unchanged.
///
/// Takes no input image: the clipboard is the only source. It is the
/// one-word spelling of "get the clipboard image", handy as the first
/// stage of a pipeline:
///
///     clip2png > save.png
///     clip2png | pngtrim > trimmed.png
///
/// The reverse direction is already covered by every tool's
/// `--output clipboard`.
#[derive(Parser)]
struct Args {
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
    let img = read_image("clipboard")?;
    write_image(&args.output, &img, args.compression)
}
