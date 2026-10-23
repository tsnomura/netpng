fn main() -> anyhow::Result<()> {
    pnginfo::run(std::env::args_os())
}
