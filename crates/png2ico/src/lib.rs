use clap::Parser;
use netpng_core::{read_image, write_ico};

/// Pack a PNG image into an ICO with one entry per size in --sizes. The
/// image is scaled to fit each square (aspect preserved) and centered on
/// transparent padding; sizes below 256 are stored as 32bpp BMP DIBs,
/// 256 as PNG-compressed.
#[derive(Parser)]
struct Args {
    /// Input path, "-" for stdin, or "clipboard".
    #[arg(long, default_value = "-")]
    input: String,
    /// Output path, "-" for stdout.
    #[arg(long, default_value = "-")]
    output: String,
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
    let img = read_image(&args.input)?;
    write_ico(&args.output, &img, &args.sizes)
}
