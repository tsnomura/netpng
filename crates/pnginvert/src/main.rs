fn main() -> anyhow::Result<()> {
    pnginvert::run(std::env::args_os())
}
