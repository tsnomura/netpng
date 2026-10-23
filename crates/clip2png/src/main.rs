fn main() -> anyhow::Result<()> {
    clip2png::run(std::env::args_os())
}
