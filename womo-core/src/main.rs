use clap::Parser;

fn main() -> anyhow::Result<()> {
    womo_core::cli::run(womo_core::cli::Cli::parse())
}
