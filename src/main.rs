use clap::Parser;

fn main() -> anyhow::Result<()> {
    nuq::run(&nuq::Args::parse())
}
