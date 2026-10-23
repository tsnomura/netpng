fn main() -> anyhow::Result<()> {
    pngflip::run(std::env::args_os())
}
