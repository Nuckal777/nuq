use std::io::Write;

use syntect::{
    easy::HighlightLines,
    highlighting::{Style, ThemeSet},
    parsing::SyntaxSet,
    util::{as_24_bit_terminal_escaped, LinesWithEndings},
};

use crate::FileFormat;

pub struct Styles {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl Default for Styles {
    fn default() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }
}

pub struct Writer<'a, W: Write> {
    buf: Vec<u8>,
    format: FileFormat,
    styles: &'a Styles,
    wrapped: W,
}

impl<'a, W: Write> Writer<'a, W> {
    pub fn new(wrapped: W, format: FileFormat, styles: &'a Styles) -> Self {
        Writer::<'a, W> {
            buf: Vec::default(),
            format,
            styles,
            wrapped,
        }
    }
}

impl<W: Write> Write for Writer<'_, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.write_highlight(self.format).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::Other, "failed to highlight output")
        })?;
        self.wrapped.flush()
    }
}

impl<W: Write> Writer<'_, W> {
    pub fn write_highlight(&mut self, format: FileFormat) -> anyhow::Result<()> {
        let extension = format.to_extension();
        let syntax = self.styles.syntax_set.find_syntax_by_extension(extension);
        if syntax.is_none() {
            self.wrapped.write_all(&self.buf)?;
            return Ok(());
        }
        let syntax = syntax.unwrap();
        let mut lighter =
            HighlightLines::new(syntax, &self.styles.theme_set.themes["InspiredGitHub"]);
        let text = std::str::from_utf8(&self.buf)?;
        for line in LinesWithEndings::from(text) {
            let ranges: Vec<(Style, &str)> = lighter
                .highlight_line(line, &self.styles.syntax_set)
                .unwrap();
            let mut escaped = as_24_bit_terminal_escaped(&ranges[..], false);
            // reset colors
            escaped.push_str("\x1b[0m");
            self.wrapped.write_all(escaped.as_bytes())?;
        }
        Ok(())
    }
}
