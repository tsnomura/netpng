fn main() -> anyhow::Result<()> {
    pngtrim::run(std::env::args_os())
}
