use clap::Parser;
use netpng_core::{read_image, write_ico};

/// Pack a PNG image into an ICO with one entry per size in --sizes. The
/// image is scaled to fit each square (aspect preserved) and centered on
/// transparent padding; sizes below 256 are stored as 32bpp BMP DIBs,
/// 256 as PNG-compressed.
#[derive(Parser)]
struct Args {
    /// Input path (PNG), "-" or omitted for stdin, or "clipboard".
    #[arg(default_value = "-")]
    input: String,
    #[command(flatten)]
    common: netpng_core::CommonArgs,
    /// Comma-separated output sizes in pixels, each 1-256.
    #[arg(long, default_value = "16,32,48,256", value_delimiter = ',')]
    sizes: Vec<u32>,
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
    for s in &args.sizes {
        anyhow::ensure!((1..=256).contains(s), "--sizes: each size must be 1-256, got {s}");
    }
    let img = read_image(&args.input)?;
    write_ico("-", &img, &args.sizes)
}
