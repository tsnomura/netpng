fn main() -> anyhow::Result<()> {
    pngchroma::run(std::env::args_os())
}
