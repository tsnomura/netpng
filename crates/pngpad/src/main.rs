fn main() -> anyhow::Result<()> {
    pngpad::run(std::env::args_os())
}
