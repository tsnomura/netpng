use clap::Parser;
use netpng_core::{gamma, read_image, write_image, CompressionLevel};

/// Apply gamma correction to the RGB channels (like pnmgamma). Alpha is
/// untouched. Each channel becomes (v/255)^(1/gamma) * 255; gamma > 1
/// brightens midtones, gamma < 1 darkens them.
#[derive(Parser)]
struct Args {
    /// Input path, "-" or omitted for stdin, or "clipboard".
    #[arg(default_value = "-")]
    input: String,
    #[arg(long)]
    gamma: f64,
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
    let out = gamma(&img, args.gamma)?;
    write_image("-", &out, args.compression)
}
