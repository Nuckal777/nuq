use clap::{Parser, ValueEnum};
use std::{
    fs::File,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};

mod highlight;

fn ext_from_path<P: AsRef<Path>>(path: P) -> anyhow::Result<String> {
    let path = path.as_ref();
    let os_ext = path
        .extension()
        .ok_or_else(|| anyhow::anyhow!("input path {} has no extension", path.display()))?;
    Ok(os_ext
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("input file extension is invalid utf-8"))?
        .to_owned())
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum FileFormat {
    Json,
    Yaml,
    Ron,
    Toml,
}

impl FileFormat {
    fn from_extension(ext: &str) -> anyhow::Result<FileFormat> {
        match ext {
            "json" | "jsonl" => Ok(FileFormat::Json),
            "ron" => Ok(FileFormat::Ron),
            "yaml" | "yml" => Ok(FileFormat::Yaml),
            "toml" => Ok(FileFormat::Toml),
            _ => Err(anyhow::anyhow!("unknown extension: {}", ext)),
        }
    }

    #[must_use]
    pub fn to_extension(self) -> &'static str {
        match self {
            FileFormat::Json => "json",
            FileFormat::Yaml => "yaml",
            FileFormat::Ron => "ron",
            FileFormat::Toml => "toml",
        }
    }

    fn read_to_json<R: Read>(self, mut reader: R) -> anyhow::Result<Vec<String>> {
        let mut json = Vec::<u8>::new();
        match self {
            FileFormat::Json => {
                let de = serde_json::Deserializer::from_reader(reader);
                let mut docs = Vec::<String>::new();
                for doc in de.into_iter::<serde_json::Value>() {
                    docs.push(doc?.to_string());
                }
                return anyhow::Ok(docs);
            }
            FileFormat::Yaml => {
                let de = serde_yaml::Deserializer::from_reader(reader);
                let mut docs = Vec::<String>::new();
                // deserializer implements iterator for multi document yamls
                for doc in de {
                    let mut buf = Vec::<u8>::new();
                    let mut se = serde_json::Serializer::new(Cursor::new(&mut buf));
                    serde_transcode::transcode(doc, &mut se)?;
                    docs.push(String::from_utf8(buf)?);
                }
                return anyhow::Ok(docs);
            }
            FileFormat::Ron => {
                let mut input = Vec::<u8>::new();
                reader.read_to_end(&mut input)?;
                let mut de = ron::Deserializer::from_bytes(&input)?;
                let mut se = serde_json::Serializer::new(Cursor::new(&mut json));
                serde_transcode::transcode(&mut de, &mut se)?;
            }
            FileFormat::Toml => {
                let mut input = Vec::<u8>::new();
                reader.read_to_end(&mut input)?;
                let toml = String::from_utf8(input)?;
                let mut de = toml::Deserializer::new(&toml);
                let mut se = serde_json::Serializer::new(Cursor::new(&mut json));
                serde_transcode::transcode(&mut de, &mut se)?;
            }
        }
        anyhow::Ok(vec![String::from_utf8(json)?])
    }

    fn write_format<W: Write>(
        self,
        values: &[String],
        pretty: bool,
        mut writer: &mut W,
    ) -> anyhow::Result<()> {
        match self {
            // need to validate that the output is actually json
            FileFormat::Json => {
                for value in values {
                    let mut de = serde_json::Deserializer::from_reader(Cursor::new(value));
                    if pretty {
                        let mut se = serde_json::Serializer::pretty(&mut writer);
                        serde_transcode::transcode(&mut de, &mut se)?;
                    } else {
                        let mut se = serde_json::Serializer::new(&mut writer);
                        serde_transcode::transcode(&mut de, &mut se)?;
                    }
                    writer.write_all(&[b'\n'])?;
                }
            }
            FileFormat::Yaml => {
                let prefix = if values.len() > 1 { "---\n" } else { "" };
                for value in values {
                    writer.write_all(prefix.as_bytes())?;
                    let mut de = serde_json::Deserializer::from_reader(Cursor::new(value));
                    let mut se = serde_yaml::Serializer::new(&mut writer);
                    serde_transcode::transcode(&mut de, &mut se)?;
                }
            }
            FileFormat::Ron => {
                // no multi document support
                if values.len() > 1 {
                    anyhow::bail!(
                        "received more than one output document, but ron does not support that."
                    );
                }
                for value in values {
                    let mut de = serde_json::Deserializer::from_reader(Cursor::new(value));
                    let pretty_conf = if pretty {
                        Some(ron::ser::PrettyConfig::default())
                    } else {
                        None
                    };
                    let mut se = ron::Serializer::with_options(
                        &mut writer,
                        pretty_conf,
                        ron::Options::default(),
                    )?;
                    serde_transcode::transcode(&mut de, &mut se)?;
                    writer.write_all(&[b'\n'])?;
                }
            }
            FileFormat::Toml => {
                // no multi document support
                if values.len() > 1 {
                    anyhow::bail!(
                        "received more than one output document, but toml does not support that."
                    );
                }
                for value in values {
                    let mut de = serde_json::Deserializer::from_reader(Cursor::new(value));
                    let mut toml = String::new();
                    let mut se = if pretty {
                        toml::Serializer::pretty(&mut toml)
                    } else {
                        toml::Serializer::new(&mut toml)
                    };
                    serde_transcode::transcode(&mut de, &mut se)?;
                    drop(se);
                    writer.write_all(toml.as_bytes())?;
                }
            }
        }
        anyhow::Ok(())
    }
}

struct JsonDocuments {
    jsons: Vec<String>,
    input_format: FileFormat,
}

impl JsonDocuments {
    fn new(jsons: Vec<String>, input_format: FileFormat) -> Self {
        Self {
            jsons,
            input_format,
        }
    }
}

struct Input {
    reader: Box<dyn Read>,
    ext: String,
    input_format: Option<FileFormat>,
}

impl Input {
    fn read_to_docs(&mut self) -> anyhow::Result<JsonDocuments> {
        if let Some(format) = self.input_format {
            return Ok(JsonDocuments::new(
                format.read_to_json(&mut self.reader)?,
                format,
            ));
        }
        if !self.ext.is_empty() {
            let format = FileFormat::from_extension(&self.ext)?;
            return Ok(JsonDocuments::new(
                format.read_to_json(&mut self.reader)?,
                format,
            ));
        }
        // guess format
        // we need to seek, so read to bytes
        let mut content = Vec::<u8>::new();
        self.reader.read_to_end(&mut content)?;
        let formats = [
            FileFormat::Json,
            FileFormat::Yaml,
            FileFormat::Toml,
            FileFormat::Ron,
        ];
        for format in formats {
            if let Ok(jsons) = format.read_to_json(Cursor::new(&content)) {
                return Ok(JsonDocuments::new(jsons, format));
            }
        }
        Err(anyhow::anyhow!("Input has an unsupported format"))
    }
}

/// A multi-format frontend for jq
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    /// Jq program to execute.
    #[clap(value_parser)]
    program: String,

    /// Input files, stdin if omitted.
    #[clap(value_parser)]
    files: Vec<PathBuf>,

    /// Input format, will be guessed by extension or content.
    #[clap(short, long, value_parser, value_enum)]
    input_format: Option<FileFormat>,

    /// Output format, if omitted will return the input format.
    /// Toml output may require reordering the input.
    #[clap(short, long, value_parser, value_enum)]
    output_format: Option<FileFormat>,

    /// If jq outputs a JSON string only output contained plain text.
    /// This post-processes the jq output, so it may not behave the same
    /// as "jq -r".
    #[clap(short, long, action)]
    raw: bool,

    /// Concatenate all input files into a JSON array before processing it
    /// with jq.
    #[clap(long, action)]
    slurp: bool,

    /// Enables or disables colored output. By default coloring is enabled
    /// when writing to a tty.
    #[clap(short, long, action)]
    color: Option<bool>,

    /// Pretty-prints the out, if the serializer supports that.
    #[clap(short, long, action)]
    pretty: bool,
}

impl Args {
    fn make_inputs(&self) -> anyhow::Result<Vec<Input>> {
        if self.files.is_empty() {
            return Ok(vec![Input {
                ext: String::new(),
                reader: Box::new(std::io::stdin()),
                input_format: self.input_format,
            }]);
        }
        let mut readers = Vec::<Input>::new();
        for path in &self.files {
            readers.push(Input {
                reader: Box::new(File::open(path)?),
                ext: ext_from_path(path)?,
                input_format: self.input_format,
            });
        }
        Ok(readers)
    }

    fn should_color(&self, format: Option<FileFormat>) -> bool {
        match self.color {
            Some(should) => should,
            None => {
                if atty::is(atty::Stream::Stdout) {
                    format.is_some()
                } else {
                    false
                }
            }
        }
    }
}

/// Turns a jq string output into a raw string
/// reverting quoting and escaping.
fn pop_quotes(text: &str) -> String {
    // check if the text starts with a quote
    // if not its likely not a string returned
    // by jq, so do nothing.
    if !text.starts_with('"') {
        return text.to_owned();
    }
    let count = text.chars().count();
    // pop first and last quote
    let mut immediate: String = text
        .char_indices()
        .filter(|(idx, char)| !(*idx == count - 2 && *char == '"'))
        .filter(|(idx, char)| !(*idx == 0 && *char == '"'))
        .map(|(_, char)| char)
        .collect();
    immediate = immediate.trim().to_owned();
    immediate.push('\n');
    // replace json escape sequence \" with "
    immediate.replace("\\\"", "\"")
}

struct Executor {
    program: jq_rs::JqProgram,
}

impl Executor {
    fn new(program: &str) -> anyhow::Result<Executor> {
        let program = jq_rs::compile(program).map_err(|err| anyhow::anyhow!("{}", err))?;
        Ok(Self { program })
    }

    fn execute<W: Write>(
        &mut self,
        jsons: &[String],
        output_format: Option<FileFormat>,
        pretty: bool,
        writer: &mut W,
    ) -> anyhow::Result<()> {
        let outputs: anyhow::Result<Vec<String>> = jsons
            .iter()
            .map(|j| {
                let output = self
                    .program
                    .run(j)
                    .map_err(|err| anyhow::anyhow!("failed to execute jq program: {}", err))?;
                match output_format {
                    Some(_) => Ok(output),
                    None => Ok(pop_quotes(&output)),
                }
            })
            .collect();
        let outputs = outputs?;
        match output_format {
            Some(format) => format
                .write_format(&outputs, pretty, writer)
                .map_err(|err| anyhow::anyhow!("failed to produce output: {}", err))?,
            None => {
                for output in outputs {
                    writer.write_all(output.as_bytes())?;
                }
            }
        }
        anyhow::Ok(())
    }
}

/// Runs nuq
/// # Errors
/// When arg parsing, io, ... fails
/// # Panics
/// When the executor is somehow not initialized.
pub fn run(args: &Args) -> anyhow::Result<()> {
    if args.raw && args.output_format.is_some() {
        anyhow::bail!("cannot use --raw with --output-format");
    }
    let inputs = if args.slurp {
        let array = slurp(&mut args.make_inputs()?)?;
        vec![Input {
            ext: String::new(),
            reader: Box::new(Cursor::new(array)),
            input_format: args.input_format,
        }]
    } else {
        args.make_inputs()?
    };
    let mut executor = Executor::new(&args.program)?;
    let styles = highlight::Styles::default();
    for mut input in inputs {
        let docs = input.read_to_docs()?;
        let output_format = if args.raw {
            None
        } else {
            Some(match args.output_format {
                Some(format) => format,
                None => docs.input_format,
            })
        };
        let mut writer: Box<dyn Write> = if args.should_color(output_format) {
            Box::new(highlight::Writer::new(
                std::io::stdout().lock(),
                output_format.unwrap(),
                &styles,
            ))
        } else {
            Box::new(std::io::stdout().lock())
        };
        match executor.execute(&docs.jsons, output_format, args.pretty, &mut writer) {
            Ok(_) => {}
            Err(err) => anyhow::bail!("{}", err),
        }
        writer.flush()?;
    }
    Ok(())
}

fn slurp(inputs: &mut [Input]) -> anyhow::Result<String> {
    let mut jsons = Vec::<String>::new();
    for input in inputs {
        jsons.extend(input.read_to_docs()?.jsons.into_iter());
    }
    let result = format!("[{}]", jsons.join(","));
    anyhow::Ok(result)
}

#[cfg(test)]
mod test {
    use std::{error::Error, io::Cursor};

    use crate::{Executor, FileFormat, Input};

    fn execute_str(
        executor: &mut Executor,
        value: &str,
        input_format: FileFormat,
        output_format: Option<FileFormat>,
    ) -> Result<String, Box<dyn Error>> {
        let jsons = input_format.read_to_json(Cursor::new(value.as_bytes()))?;
        let mut buf = Vec::<u8>::new();
        executor.execute(&jsons, output_format, false, &mut Cursor::new(&mut buf))?;
        let result = String::from_utf8(buf)?;
        Ok(result)
    }

    #[test]
    fn file_format_from_extension() {
        assert_eq!(
            FileFormat::from_extension("json").unwrap(),
            FileFormat::Json
        );
        assert_eq!(FileFormat::from_extension("ron").unwrap(), FileFormat::Ron);
        assert_eq!(
            FileFormat::from_extension("yaml").unwrap(),
            FileFormat::Yaml
        );
        assert_eq!(FileFormat::from_extension("yml").unwrap(), FileFormat::Yaml);
        assert_eq!(
            FileFormat::from_extension("toml").unwrap(),
            FileFormat::Toml
        );
        assert_eq!(
            FileFormat::from_extension("jsonl").unwrap(),
            FileFormat::Json
        );
        assert!(FileFormat::from_extension("garbage").is_err());
    }

    #[test]
    fn file_format_to_extension() {
        assert_eq!(FileFormat::Json.to_extension(), "json");
        assert_eq!(FileFormat::Yaml.to_extension(), "yaml");
        assert_eq!(FileFormat::Toml.to_extension(), "toml");
        assert_eq!(FileFormat::Ron.to_extension(), "ron");
    }

    #[test]
    fn identity_json() -> Result<(), Box<dyn Error>> {
        let json = r#"{"a":"b"}"#;
        let mut executor = Executor::new(".")?;
        let result = execute_str(
            &mut executor,
            json,
            FileFormat::Json,
            Some(FileFormat::Json),
        )?;
        assert_eq!(result, format!("{}\n", json));
        Ok(())
    }

    #[test]
    fn identity_yaml() -> Result<(), Box<dyn Error>> {
        let yaml = "a: b";
        let mut executor = Executor::new(".")?;
        let result = execute_str(
            &mut executor,
            yaml,
            FileFormat::Yaml,
            Some(FileFormat::Yaml),
        )?;
        assert_eq!(result, format!("{}\n", yaml));
        Ok(())
    }

    #[test]
    fn identity_multi_yaml() -> Result<(), Box<dyn Error>> {
        let yaml = "a: b\n---\na: c";
        let mut executor = Executor::new(".")?;
        let result = execute_str(
            &mut executor,
            yaml,
            FileFormat::Yaml,
            Some(FileFormat::Yaml),
        )?;
        assert_eq!(&result, "---\na: b\n---\na: c\n");
        Ok(())
    }

    #[test]
    fn identity_ron() -> Result<(), Box<dyn Error>> {
        let ron = r#"(a: "b")"#;
        let mut executor = Executor::new(".")?;
        let result = execute_str(&mut executor, ron, FileFormat::Ron, Some(FileFormat::Ron))?;
        assert_eq!(result, "{\"a\":\"b\"}\n");
        Ok(())
    }

    #[test]
    fn identity_toml() -> Result<(), Box<dyn Error>> {
        let ron = r#"a = "b""#;
        let mut executor = Executor::new(".")?;
        let result = execute_str(&mut executor, ron, FileFormat::Toml, Some(FileFormat::Toml))?;
        assert_eq!(result, "a = \"b\"\n");
        Ok(())
    }

    #[test]
    fn string_json() -> Result<(), Box<dyn Error>> {
        let json = r#"{"a":"b"}"#;
        let mut executor = Executor::new(".a")?;
        let result = execute_str(
            &mut executor,
            json,
            FileFormat::Json,
            Some(FileFormat::Json),
        )?;
        assert_eq!(result, "\"b\"\n");
        Ok(())
    }

    #[test]
    fn string_raw() -> Result<(), Box<dyn Error>> {
        let json = r#"{"a":"b"}"#;
        let mut executor = Executor::new(".a")?;
        let result = execute_str(&mut executor, json, FileFormat::Json, None)?;
        assert_eq!(result, "b\n");
        Ok(())
    }

    #[test]
    fn slurp() -> Result<(), Box<dyn Error>> {
        let json = Input {
            ext: String::new(),
            reader: Box::new(Cursor::new(r#"{"a":"b"}"#)),
            input_format: Some(FileFormat::Json),
        };
        let yaml = Input {
            ext: String::new(),
            reader: Box::new(Cursor::new("c: d")),
            input_format: Some(FileFormat::Yaml),
        };
        let array = super::slurp(&mut [json, yaml])?;
        assert_eq!(array, r#"[{"a":"b"},{"c":"d"}]"#);
        Ok(())
    }

    #[test]
    fn guess() {
        let mut json = Input {
            ext: String::new(),
            reader: Box::new(Cursor::new(r#"{"a":"b"}"#)),
            input_format: None,
        };
        assert!(json.read_to_docs().is_ok());
        let mut yaml = Input {
            ext: String::new(),
            reader: Box::new(Cursor::new("c: d")),
            input_format: None,
        };
        assert!(yaml.read_to_docs().is_ok());
    }
}
