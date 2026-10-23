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
#[command(version)]
struct Args {
    #[command(flatten)]
    input: netpng_core::InputArgs,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(short = 'o', long, default_value = "-")]
    output: String,
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
    let args = Args::parse_from(args);
    let input = args.input.path();
    anyhow::ensure!(
        !(input == "-" && args.mask.as_deref() == Some("-")),
        "input and --mask can't both be \"-\" (stdin can only be read once)"
    );
    let img = read_image(input)?;
    let out = match &args.mask {
        Some(mask_path) => {
            let mask = read_image(mask_path)?;
            apply_alpha(&img, &mask)?
        }
        None => extract_alpha(&img),
    };
    write_image(&args.output, &out, args.compression)
}
