fn main() -> anyhow::Result<()> {
    pngcompose::run(std::env::args_os())
}
