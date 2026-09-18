mod checker;
mod cli;
mod config;
mod diagnostic;
mod discovery;
mod output;
mod parser;
mod rules;
mod semantic;
mod source;
use clap::{CommandFactory, Parser};
use std::{
    io::{self, Write},
    process::ExitCode,
};
fn main() -> ExitCode {
    let cli = match cli::Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            let code = if e.use_stderr() { 2 } else { 0 };
            if code == 2 {
                let _ = writeln!(io::stderr(), "GSU-E001: invalid command line");
            }
            if e.print().is_err() {
                return ExitCode::from(2);
            }
            return ExitCode::from(code);
        }
    };
    let result = match cli.command {
        None => cli::Cli::command().print_help().map(|_| 0),
        Some(cli::Command::Rule { rule_id }) => match rules::metadata(&rule_id) {
            Some(rule) => writeln!(
                io::stdout(),
                "{} — {}\nSeverity: {}\nStatus: {}\n\n{}\n\n{}\n\nSuggestion: {}\n\n{}",
                rule.code,
                rule.name,
                rule.severity,
                if rule.preview { "preview" } else { "stable" },
                rule.message,
                rule.explanation,
                rule.suggestion,
                rule.details
            )
            .map(|_| 0),
            None => writeln!(
                io::stderr(),
                "GSU-E003: unknown rule {}",
                source::safe(&rule_id)
            )
            .map(|_| 2),
        },
        Some(cli::Command::Check(args)) => {
            let mut report =
                std::panic::catch_unwind(|| checker::check(&args)).unwrap_or_else(|_| {
                    let mut r = diagnostic::RunReport::default();
                    r.errors.push(diagnostic::OperationError::new(
                        "GSU-E010",
                        "Unexpected internal analysis failure",
                        None,
                    ));
                    r
                });
            report.finish();
            output::render(&report, args.output_format, args.color).map(|_| report.exit_code())
        }
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            let _ = writeln!(
                io::stderr(),
                "GSU-E009: output failed: {}",
                source::safe(&e.to_string())
            );
            ExitCode::from(2)
        }
    }
}
