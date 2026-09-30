use clap::Parser;
use netpng_core::{parse_color, read_image, rotate, write_image, CompressionLevel};

/// Rotate an image by an arbitrary angle (like pnmrotate). The canvas
/// expands to fit the whole rotated image; newly exposed corners are
/// filled with `--background` (default: transparent).
#[derive(Parser)]
struct Args {
    /// Input path, "-" for stdin, or "clipboard".
    #[arg(long, default_value = "-")]
    input: String,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(long, default_value = "-")]
    output: String,
    /// Rotation angle in degrees, clockwise.
    #[arg(long, allow_hyphen_values = true, default_value_t = 0.0)]
    angle: f64,
    /// Fill color for newly exposed corners, as "R,G,B" or "R,G,B,A".
    #[arg(long, default_value = "0,0,0,0")]
    background: String,
    #[arg(long, value_enum, default_value = "fast")]
    compression: CompressionLevel,
}

pub fn run<I, T>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let args = Args::parse_from(args);
    let background = parse_color(&args.background)?;
    let img = read_image(&args.input)?;
    let out = rotate(&img, args.angle, background);
    write_image(&args.output, &out, args.compression)
}
