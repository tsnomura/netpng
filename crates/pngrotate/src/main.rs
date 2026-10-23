fn main() -> anyhow::Result<()> {
    pngrotate::run(std::env::args_os())
}
