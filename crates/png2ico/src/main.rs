fn main() -> anyhow::Result<()> {
    png2ico::run(std::env::args_os())
}
