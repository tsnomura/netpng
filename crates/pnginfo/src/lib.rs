use clap::Parser;
use netpng_core::read_info;

/// Report basic information about a PNG image (like pamfile).
#[derive(Parser)]
struct Args {
    /// Input path, "-" for stdin, or "clipboard".
    #[arg(long, default_value = "-")]
    input: String,
}

pub fn run<I, T>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let args = Args::parse_from(args);
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
