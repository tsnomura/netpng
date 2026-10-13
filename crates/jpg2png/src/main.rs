fn main() -> anyhow::Result<()> {
    jpg2png::run(std::env::args_os())
}
