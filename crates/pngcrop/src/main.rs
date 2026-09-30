fn main() -> anyhow::Result<()> {
    pngcrop::run(std::env::args_os())
}
