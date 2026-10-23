fn main() -> anyhow::Result<()> {
    pngscale::run(std::env::args_os())
}
