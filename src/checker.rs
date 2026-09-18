use crate::{
    cli::CheckArgs,
    config::{Config, relative},
    diagnostic::{OperationError, RunReport},
    discovery, parser, semantic,
    source::Source,
};
use std::{fs::OpenOptions, io::Read, path::Path};
pub fn check(args: &CheckArgs) -> RunReport {
    let mut report = RunReport::default();
    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            report
                .errors
                .push(OperationError::new("GSU-E004", e.to_string(), None));
            return report;
        }
    };
    let config = match Config::load(args, &cwd) {
        Ok(c) => c,
        Err(e) => {
            report.errors.push(e);
            return report;
        }
    };
    let paths = if args.paths.is_empty() {
        vec![cwd.clone()]
    } else {
        args.paths.clone()
    };
    let files = discovery::discover(&paths, &cwd, &config, &mut report);
    for path in files {
        match analyze(&path, &config) {
            Ok(mut diagnostics) => {
                if report.diagnostics.len() + diagnostics.len() > 100_000 {
                    report.errors.push(OperationError::new(
                        "GSU-E008",
                        "Diagnostic limit (100000) exceeded",
                        Some(relative(&path, &config.root)),
                    ));
                    break;
                }
                report.summary.files_checked += 1;
                report.diagnostics.append(&mut diagnostics);
            }
            Err(e) => {
                report.summary.files_failed += 1;
                report.errors.push(e);
            }
        }
    }
    report
}
fn analyze(
    path: &Path,
    config: &Config,
) -> Result<Vec<crate::diagnostic::Diagnostic>, OperationError> {
    let name = relative(path, &config.root);
    let fail = |code, msg: String| OperationError::new(code, msg, Some(name.clone()));
    let meta = std::fs::symlink_metadata(path).map_err(|e| fail("GSU-E007", e.to_string()))?;
    if !meta.is_file() {
        return Err(fail(
            "GSU-E007",
            "Source is no longer a regular file".into(),
        ));
    }
    if meta.len() > 2 * 1024 * 1024 {
        return Err(fail("GSU-E008", "File exceeds 2 MiB".into()));
    }
    let mut bytes = vec![];
    {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        options.open(path)
    }
    .and_then(|f| f.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes))
    .map_err(|e| fail("GSU-E007", e.to_string()))?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(fail("GSU-E008", "File exceeds 2 MiB".into()));
    }
    let text = String::from_utf8(bytes).map_err(|_| {
        fail(
            "GSU-E005",
            "Source is not valid UTF-8; convert the file encoding before checking".into(),
        )
    })?;
    let source = Source::new(text, name.clone());
    // A dedicated bounded stack protects ordinary deep AST traversal without changing process state.
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn_scoped(scope, || {
                let ast = parser::parse(&source)?;
                semantic::analyze(&source, &ast, &config.enabled)
            })
            .map_err(|e| fail("GSU-E010", e.to_string()))?;
        worker.join().unwrap_or_else(|_| {
            Err(fail(
                "GSU-E010",
                "Unexpected parser/analysis failure".into(),
            ))
        })
    })
}
