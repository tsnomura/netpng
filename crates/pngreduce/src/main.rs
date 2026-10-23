fn main() -> anyhow::Result<()> {
    pngreduce::run(std::env::args_os())
}
