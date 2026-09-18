use crate::{
    cli::{Color, OutputFormat},
    diagnostic::RunReport,
    rules,
    source::safe,
};
use std::io::{self, IsTerminal, Write};

fn style(text: &str, color: bool, code: &str) -> String {
    if color {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.into()
    }
}

pub fn render(report: &RunReport, format: OutputFormat, color: Color) -> io::Result<()> {
    let use_color = match color {
        Color::Auto => io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
        Color::Always => true,
        Color::Never => false,
    };
    let mut out = io::BufWriter::new(io::stdout().lock());
    match format {
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut out, report)?;
            writeln!(out)?;
        }
        OutputFormat::Full | OutputFormat::Concise => {
            for d in &report.diagnostics {
                let code = style(d.rule, use_color, "1;33");
                let path = safe(&d.location.path);
                let line = d.location.start.line;
                let column = d.location.start.column;
                if matches!(format, OutputFormat::Concise) {
                    writeln!(out, "{path}:{line}:{column}: {code} {}", d.message)?;
                    continue;
                }
                let width = line.to_string().len();
                let rule = rules::metadata(d.rule).expect("registered diagnostic");
                writeln!(out, "{code} {}", d.message)?;
                writeln!(out, "  --> {path}:{line}:{column}")?;
                writeln!(out, " {:width$} |", "")?;
                writeln!(out, " {line:>width$} | {}", d.excerpt)?;
                writeln!(
                    out,
                    " {:width$} | {}",
                    "",
                    style(&d.underline, use_color, "33")
                )?;
                if d.location.end.line > line {
                    writeln!(
                        out,
                        " {:width$} | ... continues to {}:{}",
                        "", d.location.end.line, d.location.end.column
                    )?;
                }
                writeln!(out, " {:width$} |", "")?;
                writeln!(out, "   = help: {}", rule.help)?;
                writeln!(
                    out,
                    "   = severity: {}; confidence: {}{}\n",
                    d.severity,
                    d.confidence,
                    if rule.preview { "; preview" } else { "" }
                )?;
            }
            let mut err = io::BufWriter::new(io::stderr().lock());
            for e in &report.errors {
                writeln!(
                    err,
                    "{}: {}{}",
                    e.code,
                    e.path
                        .as_ref()
                        .map(|p| format!("{}: ", safe(p)))
                        .unwrap_or_default(),
                    safe(&e.message)
                )?;
            }
            for s in &report.skipped {
                writeln!(err, "Skipped {} ({})", safe(&s.path), s.reason)?;
            }
            err.flush()?;
            let plural = |count| if count == 1 { "" } else { "s" };
            writeln!(
                out,
                "Checked {} file{}; found {} diagnostic{}; {} error{}.",
                report.summary.files_checked,
                plural(report.summary.files_checked),
                report.summary.diagnostics,
                plural(report.summary.diagnostics),
                report.errors.len(),
                plural(report.errors.len())
            )?;
        }
    }
    out.flush()
}
