use clap::Parser;
use netpng_core::{read_image, write_image, CompressionLevel};

/// Copy the image on the clipboard to stdout or a file, as PNG.
///
/// Takes no input image: the clipboard is the only source. It is the
/// one-word spelling of "get the clipboard image", handy as the first
/// stage of a pipeline, e.g. `clip2png > save.png` or
/// `clip2png | pngtrim > trimmed.png`.
///
/// The clipboard hands over decoded 8-bit RGBA pixels, not a file, so the
/// output is a fresh 8-bit RGBA PNG: a 16-bit source's extra precision and
/// any PNG metadata are not carried over.
///
/// The reverse direction is every tool's `--output clipboard` (on Linux
/// that keeps the command running, serving the image, until something
/// else is copied; `pngview --copy-to-clipboard` serves it in the
/// background instead).
#[derive(Parser)]
#[command(version)]
struct Args {
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
    let img = read_image("clipboard")?;
    write_image(&args.output, &img, args.compression)
}
