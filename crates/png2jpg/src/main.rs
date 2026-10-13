fn main() -> anyhow::Result<()> {
    png2jpg::run(std::env::args_os())
}
