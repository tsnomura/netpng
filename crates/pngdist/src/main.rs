fn main() -> anyhow::Result<()> {
    pngdist::run(std::env::args_os())
}
