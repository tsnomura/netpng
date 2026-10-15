fn main() -> anyhow::Result<()> {
    pngalpha::run(std::env::args_os())
}
