mod cli;
mod commands;
mod config;
mod git;
mod store;

fn main() -> anyhow::Result<()> {
    cli::run()
}
