fn main() -> anyhow::Result<()> {
    pngblur::run(std::env::args_os())
}
