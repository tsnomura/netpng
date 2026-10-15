fn main() -> anyhow::Result<()> {
    pngflatten::run(std::env::args_os())
}
