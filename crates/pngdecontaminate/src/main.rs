fn main() -> anyhow::Result<()> {
    pngdecontaminate::run(std::env::args_os())
}
