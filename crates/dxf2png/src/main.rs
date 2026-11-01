fn main() -> anyhow::Result<()> {
    dxf2png::run(std::env::args_os())
}
