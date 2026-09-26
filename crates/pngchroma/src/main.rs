use clap::Parser;
use netpng_core::{chroma_key, parse_color, pixel_at, read_image, write_image, CompressionLevel};

/// Chroma-key a solid-color background into an alpha channel. Unlike a
/// plain binary cutout, the transition from opaque to transparent is a
/// linear ramp over the "--similarity"-to-"--similarity + --blend" RGB
/// distance range, giving an anti-aliased edge.
#[derive(Parser)]
struct Args {
    /// Input path, "-" for stdin, or "clipboard".
    #[arg(long, default_value = "-")]
    input: String,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(long, default_value = "-")]
    output: String,
    /// Background color as "R,G,B"; defaults to the top-left pixel.
    #[arg(long)]
    background: Option<String>,
    /// RGB distance below which a pixel is fully transparent.
    #[arg(long, default_value_t = 30.0)]
    similarity: f64,
    /// Width of the distance range over which alpha ramps from 0 to 255.
    #[arg(long, default_value_t = 30.0)]
    blend: f64,
    #[arg(long, value_enum, default_value = "fast")]
    compression: CompressionLevel,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    anyhow::ensure!(args.blend > 0.0, "--blend must be positive");

    let img = read_image(&args.input)?;
    let background = match &args.background {
        Some(s) => {
            let [r, g, b, _] = parse_color(s)?;
            [r, g, b]
        }
        None => {
            let [r, g, b, _] = pixel_at(&img, 0, 0);
            [r, g, b]
        }
    };

    let out = chroma_key(&img, background, args.similarity, args.blend);
    write_image(&args.output, &out, args.compression)
}
