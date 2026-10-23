fn main() -> anyhow::Result<()> {
    ico2png::run(std::env::args_os())
}
