use clap::Parser;
use netpng_core::read_info;

/// Report basic information about a PNG image (like pamfile).
#[derive(Parser)]
struct Args {
    /// Input path, "-" for stdin, or "clipboard".
    #[arg(long, default_value = "-")]
    input: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let info = read_info(&args.input)?;
    print!(
        "{}x{} color={:?} depth={:?}",
        info.width, info.height, info.color_type, info.bit_depth
    );
    if let Some(bytes) = info.source_bytes {
        print!(" bytes={bytes}");
    }
    println!();
    Ok(())
}
