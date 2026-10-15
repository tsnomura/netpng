use clap::Parser;
use netpng_core::read_info;

/// Report basic information about a PNG image (like pamfile).
#[derive(Parser)]
struct Args {
    /// Input path, "-" or omitted for stdin, or "clipboard".
    #[arg(default_value = "-")]
    input: String,
    /// Print the version and exit.
    #[arg(short = 'v', long)]
    version: bool,
    /// Suppress informational messages (errors are still printed).
    #[arg(short = 'q', long)]
    quiet: bool,
}

pub fn run<I, T>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let args = Args::parse_from(netpng_core::normalize_args(args));
    if args.version {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let _ = args.quiet;
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
