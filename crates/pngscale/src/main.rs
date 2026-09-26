use clap::Parser;
use netpng_core::{read_image, scale, write_image, CompressionLevel};

/// Resize an image (like pamscale). Give --scale, or --width and/or
/// --height (the other dimension is derived from the aspect ratio if only
/// one is given).
#[derive(Parser)]
struct Args {
    /// Input path, "-" for stdin, or "clipboard".
    #[arg(long, default_value = "-")]
    input: String,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(long, default_value = "-")]
    output: String,
    #[arg(long)]
    width: Option<u32>,
    #[arg(long)]
    height: Option<u32>,
    /// Multiply both dimensions by this factor; can't be combined with --width/--height.
    #[arg(long)]
    scale: Option<f64>,
    #[arg(long, value_enum, default_value = "fast")]
    compression: CompressionLevel,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let img = read_image(&args.input)?;

    let (out_w, out_h) = if let Some(s) = args.scale {
        anyhow::ensure!(
            args.width.is_none() && args.height.is_none(),
            "--scale can't be combined with --width/--height"
        );
        anyhow::ensure!(s > 0.0, "--scale must be positive");
        (
            ((img.width as f64) * s).round().max(1.0) as u32,
            ((img.height as f64) * s).round().max(1.0) as u32,
        )
    } else {
        match (args.width, args.height) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => (
                w,
                ((img.height as f64) * (w as f64 / img.width as f64)).round().max(1.0) as u32,
            ),
            (None, Some(h)) => (
                ((img.width as f64) * (h as f64 / img.height as f64)).round().max(1.0) as u32,
                h,
            ),
            (None, None) => anyhow::bail!("specify --scale, or --width and/or --height"),
        }
    };

    anyhow::ensure!(out_w >= 1 && out_h >= 1, "resulting dimensions must be at least 1x1");
    let out = scale(&img, out_w, out_h);
    write_image(&args.output, &out, args.compression)
}
