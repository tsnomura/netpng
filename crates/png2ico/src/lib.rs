use clap::Parser;
use netpng_core::{read_image, write_ico};

/// Pack a PNG image into an ICO with one entry per size in --sizes. The
/// image is scaled to fit each square (aspect preserved) and centered on
/// transparent padding; sizes below 256 are stored as 32bpp BMP DIBs,
/// 256 as PNG-compressed.
#[derive(Parser)]
#[command(version)]
struct Args {
    #[command(flatten)]
    input: netpng_core::InputArgs,
    /// Output path, "-" for stdout.
    #[arg(short = 'o', long, default_value = "-")]
    output: String,
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
    let args = Args::parse_from(args);
    for s in &args.sizes {
        anyhow::ensure!((1..=256).contains(s), "--sizes: each size must be 1-256, got {s}");
    }
    let input = args.input.path();
    let img = read_image(input)?;
    write_ico(&args.output, &img, &args.sizes)
}
