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

struct Input {
    reader: Box<dyn std::io::Read>,
    format: Option<FileFormat>,
}

/// A multi-format frontend for jq
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Jq program to execute.
    #[clap(value_parser)]
    program: String,

    /// Input files, stdin if omitted.
    #[clap(value_parser)]
    files: Vec<PathBuf>,

    /// Input format, will be guessed by extension if omitted.
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
    fn make_inputs(&self) -> anyhow::Result<Vec<Input>> {
        if self.files.is_empty() {
            return Ok(vec![Input {
                format: None,
                reader: Box::new(std::io::stdin().lock()),
            }]);
        }
        let mut readers = Vec::<Input>::new();
        for path in &self.files {
            readers.push(Input {
                reader: Box::new(File::open(path)?),
                format: match path.extension() {
                    Some(ext) => FileFormat::from_extension(
                        ext.to_str()
                            .ok_or_else(|| anyhow::anyhow!("input file name is invalid utf-8"))?,
                    ),
                    None => None,
                },
            });
        }
        Ok(readers)
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
    program: jq_rs::JqProgram,
    output_format: Option<FileFormat>,
    raw: bool,
}

impl Executor {
    fn new(
        program: &str,
        output_format: Option<FileFormat>,
        raw: bool,
    ) -> anyhow::Result<Executor> {
        let program = jq_rs::compile(program).map_err(|err| anyhow::anyhow!("{}", err))?;
        Ok(Self {
            program,
            output_format,
            raw,
        })
    }

    fn execute<R: std::io::Read, W: std::io::Write>(
        &mut self,
        reader: &mut R,
        writer: &mut W,
        input_format: FileFormat,
    ) -> anyhow::Result<()> {
        let json = input_format
            .read_to_json(reader)
            .map_err(|err| anyhow::anyhow!("failed to convert input to json: {}", err))?;
        let jq_output = self
            .program
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
    let mut executor = Executor::new(&args.program, args.output_format, args.raw)?;
    for mut input in args.make_inputs()? {
        let input_format = args
            .input_format
            .map_or_else(|| input.format, Some)
            .ok_or_else(|| anyhow::anyhow!("could not determine input format"))?;
        match executor.execute(
            &mut input.reader,
            &mut std::io::stdout().lock(),
            input_format,
        ) {
            Ok(_) => {}
            Err(err) => anyhow::bail!("{}", err),
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use std::{error::Error, io::Cursor};

    use crate::{Executor, FileFormat};

    fn execute_str(
        executor: &mut Executor,
        value: &str,
        input_format: FileFormat,
    ) -> Result<String, Box<dyn Error>> {
        let mut buf = Vec::<u8>::new();
        executor.execute(
            &mut Cursor::new(value),
            &mut Cursor::new(&mut buf),
            input_format,
        )?;
        let result = String::from_utf8(buf)?;
        Ok(result)
    }

    #[test]
    fn identity() -> Result<(), Box<dyn Error>> {
        let json = r#"{"a":"b"}"#;
        let mut executor = Executor::new(".", Some(FileFormat::Json), false)?;
        let result = execute_str(&mut executor, json, FileFormat::Json)?;
        assert_eq!(result, json);
        Ok(())
    }
}
