fn main() -> anyhow::Result<()> {
    pnggamma::run(std::env::args_os())
}
