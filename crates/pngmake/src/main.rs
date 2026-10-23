fn main() -> anyhow::Result<()> {
    pngmake::run(std::env::args_os())
}
