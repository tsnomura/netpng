fn main() -> anyhow::Result<()> {
    pngcat::run(std::env::args_os())
}
