fn main() -> anyhow::Result<()> {
    pngview::run(std::env::args_os())
}
