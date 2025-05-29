use crate::assets::HighlightingAssets;
use crate::config::Config;
use crate::error::*;
use crate::input::OpenedInput;
use crate::line_range::MaxBufferedLineNumber;
use crate::output::OutputHandle;
use crate::printer::Printer;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SyntectStyle, Theme};
use syntect::parsing::{ParseState, ScopeStack, SyntaxSet};
use nu_ansi_term::Style as AnsiStyle;

fn syntect_style_to_ansi(style: &SyntectStyle) -> AnsiStyle {
    let mut ansi = AnsiStyle::new();
    ansi = ansi.fg(nu_ansi_term::Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b));
    if style.font_style.contains(syntect::highlighting::FontStyle::BOLD) {
        ansi = ansi.bold();
    }
    if style.font_style.contains(syntect::highlighting::FontStyle::ITALIC) {
        ansi = ansi.italic();
    }
    if style.font_style.contains(syntect::highlighting::FontStyle::UNDERLINE) {
        ansi = ansi.underline();
    }
    ansi
}

pub struct TokenPrinter<'a> {
    syntax: &'a syntect::parsing::SyntaxReference,
    syntax_set: &'a SyntaxSet,
    theme: &'a Theme,
    wide_mode: bool,
}

impl<'a> TokenPrinter<'a> {
    pub fn new(config: &'a Config, assets: &'a HighlightingAssets, input: &mut OpenedInput, wide_mode: bool) -> Result<Self> {
        let theme = assets.get_theme(&config.theme);
        let syntax_in_set = assets.get_syntax(config.language, input, &config.syntax_mapping)?;
        Ok(TokenPrinter {
            syntax: syntax_in_set.syntax,
            syntax_set: syntax_in_set.syntax_set,
            theme,
            wide_mode,
        })
    }

    fn print_tokens_for_line(&self, handle: &mut OutputHandle, line_number: usize, line: &str) -> Result<()> {
        let mut highlighter = HighlightLines::new(self.syntax, self.theme);
        let regions = highlighter.highlight_line(line, self.syntax_set)
            .map_err(|e| Error::Msg(e.to_string()))?;

        // Use ParseState to get scope info for each region
        let mut parse_state = ParseState::new(self.syntax);
        let mut scope_stack = ScopeStack::new();
        let ops = parse_state.parse_line(line, self.syntax_set)
            .map_err(|e| Error::Msg(e.to_string()))?;

        // Build a vector of (offset, scope) for each region
        let mut scopes_by_offset = Vec::new();
        for (offset, op) in &ops {
            scope_stack.apply(op).map_err(|e| Error::Msg(e.to_string()))?;
            let scope = scope_stack.as_slice().last()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "text".to_string());
            scopes_by_offset.push((*offset, scope));
        }

        let mut col = 0;
        let mut byte_offset = 0;
        for (style, text) in regions {
            if !text.is_empty() {
                // Find the scope for this region by offset
                let scope = scopes_by_offset
                    .iter()
                    .rev()
                    .find(|(off, _)| *off <= byte_offset)
                    .map(|(_, s)| s.as_str())
                    .unwrap_or("text");
                let base_scope = scope.split('.').next().unwrap_or(scope);
                let ansi = syntect_style_to_ansi(&style);
                if self.wide_mode {
                    write!(handle, "{}:{}\t{}\t", line_number, col, ansi.paint(format!("{:<48}", scope)))?;
                } else {
                    write!(handle, "{}:{}\t{}\t", line_number, col, ansi.paint(format!("{:<10}", base_scope)))?;
                }
                write!(handle, "{}", ansi.paint(text.replace('\n', "\\n")))?;
                writeln!(handle)?;
                col += text.len();
                byte_offset += text.len();
            }
        }
        Ok(())
    }
}

impl Printer for TokenPrinter<'_> {
    fn print_header(&mut self, _handle: &mut OutputHandle, _input: &OpenedInput, _add_header_padding: bool) -> Result<()> {
        Ok(())
    }
    fn print_footer(&mut self, _handle: &mut OutputHandle, _input: &OpenedInput) -> Result<()> {
        Ok(())
    }
    fn print_snip(&mut self, _handle: &mut OutputHandle) -> Result<()> {
        Ok(())
    }
    fn print_line(&mut self, _out_of_range: bool, handle: &mut OutputHandle, line_number: usize, line_buffer: &[u8], _max_buffered_line_number: MaxBufferedLineNumber) -> Result<()> {
        let line = String::from_utf8_lossy(line_buffer);
        self.print_tokens_for_line(handle, line_number, &line)
    }
} 