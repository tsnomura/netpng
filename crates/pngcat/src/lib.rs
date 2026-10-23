use clap::{CommandFactory, FromArgMatches, Parser, ValueEnum};
use netpng_core::{read_image, write_image, CompressionLevel, Image};

#[derive(Copy, Clone, ValueEnum)]
enum Direction {
    Horizontal,
    Vertical,
}

/// Concatenate PNG images side by side (horizontal) or stacked (vertical).
/// A single input is passed through unchanged, like `cat`; with no input
/// at all, stdin is that single input.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Input images, in order; "-" for stdin (at most once; also the
    /// default when no input is given), or "clipboard".
    #[arg(value_name = "INPUTS")]
    inputs: Vec<String>,
    /// Same as an INPUTS entry, as a flag (repeatable); positional and
    /// flag inputs keep the order they were given in on the command line.
    #[arg(short = 'i', long = "input", value_name = "INPUT")]
    input: Vec<String>,
    /// Output path, "-" for stdout, or "clipboard".
    #[arg(short = 'o', long, default_value = "-")]
    output: String,
    #[command(flatten)]
    common: netpng_core::CommonArgs,
    #[arg(long, value_enum, default_value = "horizontal")]
    direction: Direction,
    #[arg(long, value_enum, default_value = "fast")]
    compression: CompressionLevel,
}

pub fn run<I, T>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    // Parsed via ArgMatches rather than Args::parse_from: clap stores
    // positional INPUTS and --input values separately, and only the
    // matches' indices tell how the two were interleaved on the command line.
    let matches = Args::command().get_matches_from(args);
    let args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

    let mut indexed: Vec<(usize, String)> = Vec::new();
    for id in ["inputs", "input"] {
        if let (Some(values), Some(indices)) =
            (matches.get_many::<String>(id), matches.indices_of(id))
        {
            indexed.extend(indices.zip(values.cloned()));
        }
    }
    indexed.sort_by_key(|&(index, _)| index);
    let mut inputs: Vec<String> = indexed.into_iter().map(|(_, path)| path).collect();
    if inputs.is_empty() {
        inputs.push("-".to_string());
    }
    let stdin_count = inputs.iter().filter(|p| p.as_str() == "-").count();
    anyhow::ensure!(stdin_count <= 1, "at most one input may be \"-\" (stdin)");

    let images: Vec<Image> = inputs
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
