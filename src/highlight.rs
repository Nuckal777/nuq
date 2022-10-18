use std::io::Write;

use syntect::{
    easy::HighlightLines,
    highlighting::{Style, ThemeSet},
    parsing::SyntaxSet,
    util::{as_24_bit_terminal_escaped, LinesWithEndings},
};

use crate::FileFormat;

pub struct Writer {
    buf: Vec<u8>,
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl Default for Writer {
    fn default() -> Self {
        Self {
            buf: Vec::default(),
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }
}

impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Writer {
    pub fn highlight(&self, format: FileFormat) -> anyhow::Result<String> {
        let extension = format.to_extension();
        let syntax = self
            .syntax_set
            .find_syntax_by_extension(extension)
            .ok_or_else(|| anyhow::anyhow!("failed to find syntax for {}", extension))?;
        let mut lighter = HighlightLines::new(syntax, &self.theme_set.themes["base16-ocean.dark"]);
        let text = std::str::from_utf8(&self.buf)?;
        let mut output = String::new();
        for line in LinesWithEndings::from(text) {
            let ranges: Vec<(Style, &str)> =
                lighter.highlight_line(line, &self.syntax_set).unwrap();
            let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
            output.push_str(&escaped);
        }
        // reset colors
        output.push_str("\x1b[0m");
        Ok(output)
    }
}
