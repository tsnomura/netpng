use clap::{Parser, ValueEnum};
use netpng_core::{read_image, write_image, CompressionLevel, Image};

#[derive(Copy, Clone, ValueEnum)]
enum Direction {
    Horizontal,
    Vertical,
}

/// Concatenate PNG images side by side (horizontal) or stacked (vertical).
#[derive(Parser)]
struct Args {
    /// Input path (repeatable, at least 2), "-" for stdin (at most once), or "clipboard".
    #[arg(long = "input", required = true)]
    inputs: Vec<String>,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(long, default_value = "-")]
    output: String,
    #[arg(long, value_enum, default_value = "horizontal")]
    direction: Direction,
    #[arg(long, value_enum, default_value = "fast")]
    compression: CompressionLevel,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    anyhow::ensure!(args.inputs.len() >= 2, "--input must be given at least twice");
    let stdin_count = args.inputs.iter().filter(|p| p.as_str() == "-").count();
    anyhow::ensure!(stdin_count <= 1, "at most one --input may be \"-\" (stdin)");

    let images: Vec<Image> = args
        .inputs
        .iter()
        .map(|p| read_image(p))
        .collect::<anyhow::Result<_>>()?;

    let out = match args.direction {
        Direction::Horizontal => cat_horizontal(&images)?,
        Direction::Vertical => cat_vertical(&images)?,
    };

    write_image(&args.output, &out, args.compression)
}

fn cat_horizontal(images: &[Image]) -> anyhow::Result<Image> {
    let height = images[0].height;
    for img in images {
        anyhow::ensure!(
            img.height == height,
            "all images must have the same height for horizontal cat (expected {height}, got {})",
            img.height
        );
    }

    let total_width: u32 = images.iter().map(|i| i.width).sum();
    let mut pixels = vec![0u8; (total_width as usize) * (height as usize) * 4];
    let mut x_offset = 0u32;
    for img in images {
        for row in 0..height {
            let src_start = (row * img.width * 4) as usize;
            let src_end = src_start + (img.width * 4) as usize;
            let dst_start = ((row * total_width + x_offset) * 4) as usize;
            let dst_end = dst_start + (img.width * 4) as usize;
            pixels[dst_start..dst_end].copy_from_slice(&img.pixels[src_start..src_end]);
        }
        x_offset += img.width;
    }

    Ok(Image {
        width: total_width,
        height,
        pixels,
    })
}

fn cat_vertical(images: &[Image]) -> anyhow::Result<Image> {
    let width = images[0].width;
    for img in images {
        anyhow::ensure!(
            img.width == width,
            "all images must have the same width for vertical cat (expected {width}, got {})",
            img.width
        );
    }

    let total_height: u32 = images.iter().map(|i| i.height).sum();
    let mut pixels = Vec::with_capacity((width as usize) * (total_height as usize) * 4);
    for img in images {
        pixels.extend_from_slice(&img.pixels);
    }

    Ok(Image {
        width,
        height: total_height,
        pixels,
    })
}
