use crate::{
    config::{Config, absolute, relative},
    diagnostic::{OperationError, RunReport, Skipped},
};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
};
const BUILTIN: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    ".venv",
    "venv",
    "__pycache__",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".gsu_cache",
    "build",
    "dist",
    "node_modules",
];
pub fn discover(
    paths: &[PathBuf],
    cwd: &Path,
    config: &Config,
    report: &mut RunReport,
) -> Vec<PathBuf> {
    let mut state = Discovery {
        config,
        report,
        seen: BTreeSet::new(),
        dirs: BTreeSet::new(),
        files: vec![],
        ignores: HashMap::new(),
        hidden_roots: paths
            .iter()
            .map(|p| absolute(p, cwd))
            .filter(|p| std::fs::symlink_metadata(p).is_ok_and(|m| m.is_dir()))
            .collect(),
        limited: false,
    };
    for input in paths {
        state.walk(&absolute(input, cwd), true, 0);
        if state.limited {
            break;
        }
    }
    state.files.sort();
    state.files
}
struct Discovery<'a> {
    config: &'a Config,
    report: &'a mut RunReport,
    seen: BTreeSet<PathBuf>,
    dirs: BTreeSet<PathBuf>,
    files: Vec<PathBuf>,
    ignores: HashMap<PathBuf, Option<Gitignore>>,
    limited: bool,
    hidden_roots: BTreeSet<PathBuf>,
}
impl Discovery<'_> {
    fn error(&mut self, code: &'static str, msg: impl Into<String>, path: &Path) {
        self.report.errors.push(OperationError::new(
            code,
            msg,
            Some(relative(path, &self.config.root)),
        ));
    }
    fn filter(&mut self, path: &Path, is_dir: bool, explicit_root: bool) -> Option<&'static str> {
        let rel = path.strip_prefix(&self.config.root).unwrap_or(path);
        let mut prefix = if path.starts_with(&self.config.root) {
            self.config.root.clone()
        } else {
            PathBuf::new()
        };
        for component in rel.components() {
            prefix.push(component.as_os_str());
            let name = component.as_os_str().to_string_lossy();
            if BUILTIN.contains(&name.as_ref()) {
                return Some("excluded");
            }
            if name.starts_with('.')
                && name != ".."
                && name != "."
                && !self.hidden_roots.contains(&prefix)
                && !(explicit_root && is_dir && path.file_name() == Some(component.as_os_str()))
            {
                return Some("hidden");
            }
        }
        if self.config.excluded(path) {
            return Some("excluded");
        }
        if path.starts_with(&self.config.root) {
            let mut parents = path
                .parent()
                .into_iter()
                .flat_map(Path::ancestors)
                .take_while(|p| p.starts_with(&self.config.root))
                .collect::<Vec<_>>();
            parents.reverse();
            let mut ignored = false;
            for parent in parents {
                if !self.ignores.contains_key(parent) {
                    let file = parent.join(".gitignore");
                    let mut matcher = None;
                    if file.exists() {
                        if std::fs::symlink_metadata(&file)
                            .is_ok_and(|m| m.file_type().is_symlink())
                        {
                            self.error("GSU-E007", "Refusing symbolic-link ignore file", &file);
                        } else {
                            let mut builder = GitignoreBuilder::new(parent);
                            if let Some(e) = builder.add(&file) {
                                self.error("GSU-E007", e.to_string(), &file);
                            } else {
                                match builder.build() {
                                    Ok(m) => matcher = Some(m),
                                    Err(e) => self.error("GSU-E007", e.to_string(), &file),
                                }
                            }
                        }
                    }
                    self.ignores.insert(parent.to_path_buf(), matcher);
                }
                if let Some(Some(matcher)) = self.ignores.get(parent) {
                    let matched = matcher.matched_path_or_any_parents(path, is_dir);
                    if matched.is_ignore() {
                        ignored = true;
                    } else if matched.is_whitelist() {
                        ignored = false;
                    }
                }
            }
            if ignored {
                return Some("excluded");
            }
        }
        None
    }
    fn walk(&mut self, path: &Path, explicit: bool, depth: usize) {
        if self.limited {
            return;
        }
        if depth > 256 {
            self.error("GSU-E008", "Directory nesting exceeds 256", path);
            return;
        }
        if explicit
            && path
                .ancestors()
                .skip(1)
                .any(|p| std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()))
        {
            self.error("GSU-E004", "Input path traverses a symbolic link", path);
            return;
        }
        let py = path.extension().is_some_and(|s| s == "py");
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(m) => m,
            Err(e) => {
                self.error(
                    if explicit { "GSU-E004" } else { "GSU-E007" },
                    e.to_string(),
                    path,
                );
                return;
            }
        };
        if metadata.is_dir() {
            if self.filter(path, true, explicit).is_some() || !self.dirs.insert(path.to_path_buf())
            {
                return;
            }
            let entries = match std::fs::read_dir(path) {
                Ok(e) => e,
                Err(e) => {
                    self.error("GSU-E007", e.to_string(), path);
                    return;
                }
            };
            let mut children = vec![];
            for entry in entries {
                match entry {
                    Ok(e) => children.push(e.path()),
                    Err(e) => self.error("GSU-E007", e.to_string(), path),
                }
            }
            children.sort();
            for child in children {
                self.walk(&child, false, depth + 1);
                if self.limited {
                    break;
                }
            }
            return;
        }
        if !py {
            if explicit {
                self.error(
                    "GSU-E004",
                    "Input must be a regular .py file or directory",
                    path,
                );
            }
            return;
        }
        if !self.seen.insert(path.to_path_buf()) {
            return;
        }
        if self.report.summary.files_discovered == 100_000 {
            self.error("GSU-E008", "Candidate file limit (100000) exceeded", path);
            self.limited = true;
            return;
        }
        self.report.summary.files_discovered += 1;
        if metadata.file_type().is_symlink() {
            if explicit {
                self.report.summary.files_failed += 1;
                self.error("GSU-E004", "Explicit symbolic links are unsupported", path);
            } else {
                self.report.skipped.push(Skipped {
                    path: relative(path, &self.config.root),
                    reason: "symlink",
                });
            }
            return;
        }
        if !metadata.is_file() {
            self.report.summary.files_failed += 1;
            self.error("GSU-E004", "Input is not a regular file", path);
            return;
        }
        // Refuse links in parent components as well as the final component.
        if path
            .ancestors()
            .skip(1)
            .any(|p| std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()))
        {
            self.report.summary.files_failed += 1;
            self.error("GSU-E004", "Input path traverses a symbolic link", path);
            return;
        }
        if let Some(reason) = self.filter(path, false, false) {
            self.report.skipped.push(Skipped {
                path: relative(path, &self.config.root),
                reason,
            });
            return;
        }
        self.files.push(path.to_path_buf());
    }
}
