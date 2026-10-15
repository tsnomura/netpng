use clap::Parser;
use netpng_core::{flatten, parse_color, read_image, write_image, CompressionLevel};

/// Flatten transparency onto a solid background color (like pnmflatten),
/// producing a fully-opaque PNG. Unlike png2jpg, the output stays PNG.
#[derive(Parser)]
struct Args {
    /// Input path, "-" or omitted for stdin, or "clipboard".
    #[arg(default_value = "-")]
    input: String,
    /// Background color as "R,G,B" to flatten alpha onto.
    #[arg(long, default_value = "255,255,255")]
    background: String,
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
    let [r, g, b, _] = parse_color(&args.background)?;
    let img = read_image(&args.input)?;
    let out = flatten(&img, [r, g, b]);
    write_image("-", &out, args.compression)
}
