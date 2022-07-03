use clap::{Parser, ErrorKind};

fn main() -> anyhow::Result<()> {
    let parse_result = nuq::Args::try_parse();
    let args = match parse_result {
        Err(err) => match err.kind() {
            ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                return {
                    err.print()?;
                    anyhow::Ok(())
                }
            }
            _ => anyhow::bail!("{}", err),
        },
        Ok(args) => args,
    };
    nuq::run(&args)
}
