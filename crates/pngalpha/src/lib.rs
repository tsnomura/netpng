use clap::Parser;
use netpng_core::{apply_alpha, extract_alpha, read_image, write_image, CompressionLevel};

/// Extract or apply an alpha channel (like netpbm's `pngtopnm -alpha` /
/// `pnmtopng -alpha=`, which use a separate grayscale image for alpha
/// since PNM itself has no alpha channel).
///
/// With no --mask, extracts the image's existing alpha channel as a
/// fully-opaque grayscale image. With --mask, replaces the image's alpha
/// channel with the mask's luma instead (the mask must be the same size
/// as the image).
#[derive(Parser)]
struct Args {
    /// Input path, "-" or omitted for stdin, or "clipboard".
    #[arg(default_value = "-")]
    input: String,
    /// Grayscale mask to apply as the new alpha channel, same size as input.
    #[arg(long)]
    mask: Option<String>,
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
    let img = read_image(&args.input)?;
    let out = match &args.mask {
        Some(mask_path) => {
            let mask = read_image(mask_path)?;
            apply_alpha(&img, &mask)?
        }
        None => extract_alpha(&img),
    };
    write_image("-", &out, args.compression)
}
