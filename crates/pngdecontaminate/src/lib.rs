use clap::Parser;
use netpng_core::{decontaminate, parse_color, read_image, write_image, CompressionLevel};

/// Remove known-background color spill from an already alpha-keyed image
/// (e.g. the output of pngchroma), reconstructing each partially
/// transparent pixel's true foreground color given the same background
/// color used to key it. Fully opaque/transparent pixels are untouched.
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
    /// The background color that was keyed out, as "R,G,B".
    #[arg(long)]
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
    let [r, g, b, _] = parse_color(&args.background)?;
    let input = args.input_flag.as_deref().unwrap_or(&args.input);
    let img = read_image(input)?;
    let out = decontaminate(&img, [r, g, b]);
    write_image(&args.output, &out, args.compression)
}
