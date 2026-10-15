use clap::Parser;
use netpng_core::{parse_color, read_image, write_jpeg};

/// Convert a PNG image to baseline JPEG (like pnmtojpeg). JPEG has no
/// alpha channel, so the image is first flattened onto --background
/// (default: white).
#[derive(Parser)]
struct Args {
    /// Input path (PNG), "-" or omitted for stdin, or "clipboard".
    #[arg(default_value = "-")]
    input: String,
    #[command(flatten)]
    common: netpng_core::CommonArgs,
    /// Background color as "R,G,B" to flatten alpha onto.
    #[arg(long, default_value = "255,255,255")]
    background: String,
    /// JPEG quality, 1 (worst) to 100 (best).
    #[arg(long, default_value_t = 90)]
    quality: u8,
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
    anyhow::ensure!((1..=100).contains(&args.quality), "--quality must be 1-100");

    let img = read_image(&args.input)?;
    let [r, g, b, _] = parse_color(&args.background)?;
    write_jpeg("-", &img, [r, g, b], args.quality)
}
