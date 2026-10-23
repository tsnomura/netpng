use clap::Parser;
use netpng_core::{parse_color, solid, write_image, CompressionLevel};

/// Create a new solid-color PNG of a given size (like pnmmake). Unlike
/// every other netpng tool, this one takes no input image.
#[derive(Parser)]
struct Args {
    #[arg(long)]
    width: u32,
    #[arg(long)]
    height: u32,
    /// Fill color as "R,G,B" or "R,G,B,A".
    #[arg(long, default_value = "255,255,255,255")]
    color: String,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(long, default_value = "-")]
    output: String,
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
    anyhow::ensure!(args.width > 0 && args.height > 0, "--width and --height must be greater than 0");
    let color = parse_color(&args.color)?;
    let img = solid(args.width, args.height, color);
    write_image(&args.output, &img, args.compression)
}
