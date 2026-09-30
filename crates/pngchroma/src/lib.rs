use clap::Parser;
use netpng_core::{chroma_key, parse_color, pixel_at, read_image, write_image, CompressionLevel};

/// Chroma-key a solid-color background into an alpha channel. Unlike a
/// plain binary cutout, the transition from opaque to transparent is a
/// linear ramp over the "--similarity"-to-"--similarity + --blend" RGB
/// distance range, giving an anti-aliased edge.
///
/// Tip: --blend needs to span roughly the full RGB distance between the
/// background and whatever foreground colors sit right at its edge, or
/// most genuinely-blended edge pixels get misclassified as fully opaque
/// (and so never get partial alpha at all). A background close in hue to
/// nearby foreground colors can use a narrow band (tens); a saturated
/// green/blue screen against very different foreground colors (e.g. red)
/// may need several hundred to capture the whole antialiasing gradient.
/// If edges still look too hard-cut, widen --blend; if too much of the
/// image fades out, narrow it (or raise --similarity). Pipe the result
/// through pngdecontaminate with the same --background to remove the
/// background color's tint from those partially transparent edge pixels.
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
    /// Widen this (e.g. to 200-300) when the background and foreground
    /// colors are far apart in RGB space, or edge pixels will jump
    /// straight from transparent to fully opaque instead of blending.
    #[arg(long, default_value_t = 30.0)]
    blend: f64,
    #[arg(long, value_enum, default_value = "fast")]
    compression: CompressionLevel,
}

pub fn run<I, T>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let args = Args::parse_from(args);
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
