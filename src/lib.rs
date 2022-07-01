use clap::{clap_derive::ArgEnum, ErrorKind, Parser};
use std::{fs::File, io::Cursor, path::PathBuf};

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
enum FileFormat {
    Json,
    Yaml,
    Ron,
}

impl FileFormat {
    fn from_extension(ext: &str) -> Option<FileFormat> {
        match ext {
            "json" => Some(FileFormat::Json),
            "ron" => Some(FileFormat::Ron),
            "yaml" | "yml" => Some(FileFormat::Yaml),
            _ => None,
        }
    }

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
    /// Jq program to execute.
    #[clap(value_parser)]
    program: String,

    /// Input file, stdin if omitted.
    #[clap(value_parser)]
    file: Option<PathBuf>,

    /// Input format, will be guessed by extension of not provided.
    #[clap(short, long, value_parser, arg_enum)]
    input_format: Option<FileFormat>,

    /// Output format, if omitted will return whatever libjq produces.
    #[clap(short, long, value_parser, arg_enum)]
    output_format: Option<FileFormat>,

    /// If jq outputs a JSON string only output contained plain text.
    /// This post-processes the jq output, so it may not behave the same
    /// as "jq -r".
    #[clap(short, long, action)]
    raw: bool,
}

impl Args {
    fn make_reader(&self) -> anyhow::Result<Box<dyn std::io::Read>> {
        match &self.file {
            Some(path) => Ok(Box::new(File::open(path)?)),
            None => Ok(Box::new(std::io::stdin().lock())),
        }
    }

    fn get_extension(&self) -> Option<&str> {
        match &self.file {
            Some(path) => match path.extension() {
                Some(ext) => ext.to_str(),
                None => None,
            },
            None => None,
        }
    }
}

fn pop_quotes(text: &str) -> String {
    // check if the text starts with a quote
    // if not its likely not a string returned
    // by jq, so do nothing.
    if !text.starts_with('"') {
        return text.to_owned();
    }
    let count = text.chars().count();
    // pop first and last quote
    let immediate: String = text
        .char_indices()
        .filter(|(idx, char)| !(*idx == count - 2 && *char == '"'))
        .filter(|(idx, char)| !(*idx == 0 && *char == '"'))
        .map(|(_, char)| char)
        .collect();
    // replace json escape sequence \" with "
    immediate.replace("\\\"", "\"")
}

struct Executor {
    program: String,
    input_format: FileFormat,
    output_format: Option<FileFormat>,
    raw: bool,
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
        let output = if self.raw {
            pop_quotes(&jq_output)
        } else {
            jq_output
        };
        match self.output_format {
            Some(format) => format
                .write_format(output, writer)
                .map_err(|err| anyhow::anyhow!("failed to produce output: {}", err))?,
            None => writer.write_all(output.as_bytes())?,
        }
        anyhow::Ok(())
    }
}

/// Runs nuq
/// # Errors
/// When arg parsing, io, ... fails
pub fn run() -> anyhow::Result<()> {
    let parse_result = Args::try_parse();
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
    if args.raw && args.output_format.is_some() {
        anyhow::bail!("cannot use --raw with --output-format");
    }
    let mut reader = match args.make_reader() {
        Ok(reader) => reader,
        Err(err) => anyhow::bail!("failed to open input: {}", err),
    };
    let executor = Executor {
        input_format: args
            .input_format
            .map_or_else(
                || match args.get_extension() {
                    Some(ext) => FileFormat::from_extension(ext),
                    None => None,
                },
                Some,
            )
            .ok_or_else(|| anyhow::anyhow!("could not determine input format"))?,
        output_format: args.output_format,
        program: args.program,
        raw: args.raw,
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
            raw: false,
        };
        let result = execute_str(&executor, json)?;
        assert_eq!(result, json);
        Ok(())
    }
}
