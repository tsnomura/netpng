use clap::Parser;
use netpng_core::{parse_color, read_image, rotate, write_image, CompressionLevel};

/// Rotate an image by an arbitrary angle (like pnmrotate). The canvas
/// expands to fit the whole rotated image; newly exposed corners are
/// filled with `--background` (default: transparent).
#[derive(Parser)]
struct Args {
    /// Input path, "-" or omitted for stdin, or "clipboard".
    #[arg(default_value = "-", value_name = "INPUT")]
    input: String,
    /// Same as INPUT, as a flag; takes precedence over the positional
    /// argument when both are given.
    #[arg(long = "input", value_name = "INPUT")]
    input_flag: Option<String>,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(long, default_value = "-")]
    output: String,
    #[command(flatten)]
    common: netpng_core::CommonArgs,
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
    let argv = netpng_core::normalize_args(args);
    if netpng_core::print_version_if_requested(&argv, env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")) {
        return Ok(());
    }
    let args = Args::parse_from(argv);
    let _ = args.common.quiet;
    let background = parse_color(&args.background)?;
    let input = args.input_flag.as_deref().unwrap_or(&args.input);
    let img = read_image(input)?;
    let out = rotate(&img, args.angle, background);
    write_image(&args.output, &out, args.compression)
}
