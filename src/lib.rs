use clap::{clap_derive::ArgEnum, Parser};
use std::{fs::File, io::Cursor, path::PathBuf};

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
enum FileFormat {
    Json,
    Yaml,
    Ron,
}

impl FileFormat {
    fn read_to_json<R: std::io::Read>(self, mut reader: R) -> anyhow::Result<String> {
        let mut json = Vec::<u8>::new();
        match self {
            FileFormat::Json => {
                reader.read_to_end(&mut json)?;
            }
            FileFormat::Yaml => {
                let de = serde_yaml::Deserializer::from_reader(reader);
                let mut se = serde_json::Serializer::new(Cursor::new(&mut json));
                serde_transcode::transcode(de, &mut se)?;
            }
            FileFormat::Ron => {
                let mut input = Vec::<u8>::new();
                reader.read_to_end(&mut input)?;
                let mut de = ron::Deserializer::from_bytes(&input)?;
                let mut se = serde_json::Serializer::new(Cursor::new(&mut json));
                serde_transcode::transcode(&mut de, &mut se)?;
            }
        }
        anyhow::Ok(String::from_utf8(json)?)
    }

    fn write_format<W: std::io::Write>(self, value: String, writer: W) -> anyhow::Result<()> {
        match self {
            // need to validate that the output is actually json
            FileFormat::Json => {
                let mut de = serde_json::Deserializer::from_reader(Cursor::new(value));
                let mut se = serde_json::Serializer::new(writer);
                serde_transcode::transcode(&mut de, &mut se)?;
            }
            FileFormat::Yaml => {
                let mut de = serde_json::Deserializer::from_reader(Cursor::new(value));
                let mut se = serde_yaml::Serializer::new(writer);
                serde_transcode::transcode(&mut de, &mut se)?;
            }
            FileFormat::Ron => {
                let mut de = serde_json::Deserializer::from_reader(Cursor::new(value));
                let mut se = ron::Serializer::with_options(writer, None, ron::Options::default())?;
                serde_transcode::transcode(&mut de, &mut se)?;
            }
        }
        anyhow::Ok(())
    }
}

/// A multi-format frontend for jq
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Jq program to execute
    #[clap(value_parser)]
    program: String,

    /// Input file
    #[clap(value_parser)]
    file: Option<PathBuf>,

    /// Input format
    #[clap(short, long, value_parser, arg_enum)]
    input_format: FileFormat,

    /// Output format
    #[clap(short, long, value_parser, arg_enum)]
    output_format: Option<FileFormat>,
}

impl Args {
    fn make_reader(&self) -> anyhow::Result<Box<dyn std::io::Read>> {
        match &self.file {
            Some(path) => Ok(Box::new(File::open(path)?)),
            None => Ok(Box::new(std::io::stdin().lock())),
        }
    }
}

struct Executor {
    program: String,
    input_format: FileFormat,
    output_format: Option<FileFormat>,
}

impl Executor {
    fn execute<R: std::io::Read, W: std::io::Write>(
        &self,
        reader: &mut R,
        writer: &mut W,
    ) -> anyhow::Result<()> {
        let json = self
            .input_format
            .read_to_json(reader)
            .map_err(|err| anyhow::anyhow!("failed to convert input to json: {}", err))?;
        let mut program =
            jq_rs::compile(&self.program).map_err(|err| anyhow::anyhow!("{}", err))?;
        let jq_output = program
            .run(&json)
            .map_err(|err| anyhow::anyhow!("failed to execute jq program: {}", err))?;
        match self.output_format {
            Some(format) => format
                .write_format(jq_output, writer)
                .map_err(|err| anyhow::anyhow!("failed to produce output: {}", err))?,
            None => writer.write_all(jq_output.as_bytes())?,
        }
        anyhow::Ok(())
    }
}

/// Runs nuq
/// # Errors
/// When arg parsing, io, ... fails
pub fn run() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut reader = match args.make_reader() {
        Ok(reader) => reader,
        Err(err) => anyhow::bail!("failed to open input: {}", err),
    };
    let executor = Executor {
        input_format: args.input_format,
        output_format: args.output_format,
        program: args.program,
    };
    match executor.execute(&mut reader, &mut std::io::stdout().lock()) {
        Ok(_) => Ok(()),
        Err(err) => anyhow::bail!("{}", err),
    }
}

#[cfg(test)]
mod test {
    use std::{error::Error, io::Cursor};

    use crate::{Executor, FileFormat};

    fn execute_str(executor: &Executor, value: &str) -> Result<String, Box<dyn Error>> {
        let mut buf = Vec::<u8>::new();
        executor.execute(&mut Cursor::new(value), &mut Cursor::new(&mut buf))?;
        let result = String::from_utf8(buf)?;
        Ok(result)
    }

    #[test]
    fn identity() -> Result<(), Box<dyn Error>> {
        let json = r#"{"a":"b"}"#;
        let executor = Executor {
            input_format: FileFormat::Json,
            output_format: Some(FileFormat::Json),
            program: ".".to_owned(),
        };
        let result = execute_str(&executor, json)?;
        assert_eq!(result, json);
        Ok(())
    }
}
