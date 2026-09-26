use clap::Parser;
use netpng_core::{crop, parse_color, pixel_at, read_image, write_image, CompressionLevel};

/// Auto-detect and remove a uniform-color border (like pnmcrop).
///
/// The background color defaults to the top-left pixel; the image is
/// trimmed to the bounding box of every pixel that differs from it.
#[derive(Parser)]
struct Args {
    /// Input path, "-" for stdin, or "clipboard".
    #[arg(long, default_value = "-")]
    input: String,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(long, default_value = "-")]
    output: String,
    #[arg(long, value_enum, default_value = "fast")]
    compression: CompressionLevel,

    /// Background color as "R,G,B" or "R,G,B,A"; defaults to the top-left pixel.
    #[arg(long)]
    background: Option<String>,
    /// Per-channel tolerance (0-255) for matching the background color.
    #[arg(long, default_value_t = 0)]
    fuzz: u8,
    /// Keep this many background pixels as a margin around the trimmed content.
    #[arg(long, default_value_t = 0)]
    margin: u32,

    /// Trim the top edge. If none of --top/--bottom/--left/--right is given, all four are trimmed.
    #[arg(long)]
    top: bool,
    #[arg(long)]
    bottom: bool,
    #[arg(long)]
    left: bool,
    #[arg(long)]
    right: bool,
}

fn channels_match(a: [u8; 4], b: [u8; 4], fuzz: u8) -> bool {
    a.iter().zip(b.iter()).all(|(&x, &y)| x.abs_diff(y) <= fuzz)
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let img = read_image(&args.input)?;

    let bg = match &args.background {
        Some(s) => parse_color(s)?,
        None => pixel_at(&img, 0, 0),
    };

    let mut bounds: Option<(u32, u32, u32, u32)> = None; // min_x, max_x, min_y, max_y
    for y in 0..img.height {
        for x in 0..img.width {
            if !channels_match(pixel_at(&img, x, y), bg, args.fuzz) {
                bounds = Some(match bounds {
                    None => (x, x, y, y),
                    Some((min_x, max_x, min_y, max_y)) => {
                        (min_x.min(x), max_x.max(x), min_y.min(y), max_y.max(y))
                    }
                });
            }
        }
    }
    let (min_x, max_x, min_y, max_y) = bounds.ok_or_else(|| {
        anyhow::anyhow!(
            "image is entirely background color {bg:?}; nothing to trim (use --background/--fuzz to change what counts as background)"
        )
    })?;

    let any_side = args.top || args.bottom || args.left || args.right;
    let (trim_top, trim_bottom, trim_left, trim_right) = if any_side {
        (args.top, args.bottom, args.left, args.right)
    } else {
        (true, true, true, true)
    };

    let left = if trim_left { min_x } else { 0 };
    let right = if trim_right { max_x + 1 } else { img.width };
    let top = if trim_top { min_y } else { 0 };
    let bottom = if trim_bottom { max_y + 1 } else { img.height };

    let left = left.saturating_sub(args.margin);
    let top = top.saturating_sub(args.margin);
    let right = right.saturating_add(args.margin).min(img.width);
    let bottom = bottom.saturating_add(args.margin).min(img.height);

    let trimmed = crop(&img, left, top, right - left, bottom - top)?;
    write_image(&args.output, &trimmed, args.compression)
}
