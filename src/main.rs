use anyhow::Result;
use clap::Parser as _;
use dedup::Args;

fn main() -> Result<()> {
    let args = Args::parse();
    dedup::run(args)
}
