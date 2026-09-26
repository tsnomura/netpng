use clap::Parser;
use netpng_core::{read_image, write_image, CompressionLevel, Image};

/// Crop a PNG image to a rectangle.
#[derive(Parser)]
struct Args {
    /// Input path, "-" for stdin, or "clipboard".
    #[arg(long, default_value = "-")]
    input: String,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(long, default_value = "-")]
    output: String,
    #[arg(long)]
    x: u32,
    #[arg(long)]
    y: u32,
    #[arg(long)]
    width: u32,
    #[arg(long)]
    height: u32,
    #[arg(long, value_enum, default_value = "fast")]
    compression: CompressionLevel,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let img = read_image(&args.input)?;

    anyhow::ensure!(
        args.x.checked_add(args.width).is_some_and(|r| r <= img.width)
            && args.y.checked_add(args.height).is_some_and(|r| r <= img.height),
        "crop rectangle ({}, {}, {}x{}) is out of bounds for a {}x{} image",
        args.x,
        args.y,
        args.width,
        args.height,
        img.width,
        img.height
    );

    let mut pixels = vec![0u8; (args.width * args.height * 4) as usize];
    for row in 0..args.height {
        let src_start = (((args.y + row) * img.width + args.x) * 4) as usize;
        let src_end = src_start + (args.width * 4) as usize;
        let dst_start = (row * args.width * 4) as usize;
        let dst_end = dst_start + (args.width * 4) as usize;
        pixels[dst_start..dst_end].copy_from_slice(&img.pixels[src_start..src_end]);
    }

    let cropped = Image {
        width: args.width,
        height: args.height,
        pixels,
    };
    write_image(&args.output, &cropped, args.compression)
}
