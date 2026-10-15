use clap::Parser;
use netpng_core::read_info;

/// Report basic information about a PNG image (like pamfile).
#[derive(Parser)]
struct Args {
    /// Input path, "-" or omitted for stdin, or "clipboard".
    #[arg(default_value = "-")]
    input: String,
    #[command(flatten)]
    common: netpng_core::CommonArgs,
}

pub fn run<I, T>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let argv = netpng_core::normalize_args(args);
    if netpng_core::print_version_if_requested(&argv, env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")) {
        return Ok(());
    }
    let args = Args::parse_from(argv);
    let _ = args.common.quiet;
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
