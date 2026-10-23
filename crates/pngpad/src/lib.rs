use clap::Parser;
use netpng_core::{pad, parse_color, read_image, write_image, CompressionLevel};

/// Add a border of a solid color around an image (like pnmpad). The
/// opposite of pngcrop; original pixels (including their own
/// transparency) are copied in unchanged.
#[derive(Parser)]
#[command(version)]
struct Args {
    #[command(flatten)]
    input: netpng_core::InputArgs,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(short = 'o', long, default_value = "-")]
    output: String,
    #[command(flatten)]
    common: netpng_core::CommonArgs,
    #[arg(long, value_enum, default_value = "fast")]
    compression: CompressionLevel,

    #[arg(long, default_value_t = 0)]
    top: u32,
    #[arg(long, default_value_t = 0)]
    bottom: u32,
    #[arg(long, default_value_t = 0)]
    left: u32,
    #[arg(long, default_value_t = 0)]
    right: u32,
    /// Border color as "R,G,B" or "R,G,B,A"; defaults to transparent.
    #[arg(long, default_value = "0,0,0,0")]
    color: String,
}

pub fn run<I, T>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let args = Args::parse_from(args);
    anyhow::ensure!(
        args.top > 0 || args.bottom > 0 || args.left > 0 || args.right > 0,
        "at least one of --top/--bottom/--left/--right must be greater than 0"
    );
    let color = parse_color(&args.color)?;
    let input = args.input.path();
    let img = read_image(input)?;
    let out = pad(&img, args.top, args.bottom, args.left, args.right, color)?;
    write_image(&args.output, &out, args.compression)
}
