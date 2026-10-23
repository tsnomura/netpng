use clap::Parser;
use netpng_core::{gamma, read_image, write_image, CompressionLevel};

/// Apply gamma correction to the RGB channels (like pnmgamma). Alpha is
/// untouched. Each channel becomes (v/255)^(1/gamma) * 255; gamma > 1
/// brightens midtones, gamma < 1 darkens them.
#[derive(Parser)]
#[command(version)]
struct Args {
    #[command(flatten)]
    input: netpng_core::InputArgs,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(short = 'o', long, default_value = "-")]
    output: String,
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
    let args = Args::parse_from(args);
    let input = args.input.path();
    let img = read_image(input)?;
    let out = gamma(&img, args.gamma)?;
    write_image(&args.output, &out, args.compression)
}
