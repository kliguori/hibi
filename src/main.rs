mod cli;
mod commands;
mod config;
mod store;

fn main() -> anyhow::Result<()> {
    cli::run()
}
