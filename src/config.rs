use crate::{cli::CheckArgs, diagnostic::OperationError, rules::RULES};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    preview: Option<bool>,
    select: Option<Vec<String>>,
    ignore: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
}
pub struct Config {
    pub root: PathBuf,
    pub enabled: BTreeSet<String>,
    pub excludes: GlobSet,
    pub components: Vec<String>,
}
pub fn relative(path: &Path, root: &Path) -> String {
    pathdiff::diff_paths(path, root)
        .unwrap_or_else(|| path.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
}
pub fn absolute(path: &Path, cwd: &Path) -> PathBuf {
    let full = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    let mut out = PathBuf::new();
    for c in full.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            _ => out.push(c),
        }
    }
    out
}
fn read(path: &Path, root: &Path) -> Result<Option<Settings>, OperationError> {
    let fail = |s: String| OperationError::new("GSU-E002", s, Some(relative(path, root)));
    let content = std::fs::read_to_string(path).map_err(|e| fail(e.to_string()))?;
    let value: toml::Value = toml::from_str(&content).map_err(|e| fail(e.to_string()))?;
    value
        .get("tool")
        .and_then(|v| v.get("gsu"))
        .map(|v| {
            v.clone()
                .try_into()
                .map_err(|e: toml::de::Error| fail(e.to_string()))
        })
        .transpose()
}
fn expand(
    tokens: Vec<String>,
    preview: bool,
    selecting: bool,
) -> Result<BTreeSet<String>, OperationError> {
    let mut result = BTreeSet::new();
    for token in tokens {
        let exact = RULES.iter().find(|r| r.code == token);
        if exact.is_none() && !RULES.iter().any(|r| r.category == token) {
            return Err(OperationError::new(
                "GSU-E003",
                format!("Unknown rule selector: {token:?}"),
                None,
            ));
        }
        if selecting && !preview && exact.is_some_and(|r| r.preview) {
            return Err(OperationError::new(
                "GSU-E003",
                format!("Rule {token} requires --preview"),
                None,
            ));
        }
        result.extend(
            RULES
                .iter()
                .filter(|r| {
                    (r.code == token || r.category == token)
                        && (!selecting || preview || !r.preview)
                })
                .map(|r| r.code.to_string()),
        );
    }
    Ok(result)
}
fn cli_list(value: &str) -> Vec<String> {
    if value.is_empty() {
        vec![]
    } else {
        value.split(',').map(str::to_string).collect()
    }
}
impl Config {
    pub fn load(args: &CheckArgs, cwd: &Path) -> Result<Self, OperationError> {
        let mut root = cwd.to_path_buf();
        let mut settings = Settings::default();
        if let Some(path) = &args.config {
            let path = absolute(path, cwd);
            root = path.parent().unwrap_or(cwd).to_path_buf();
            settings = read(&path, &root)?.ok_or_else(|| {
                OperationError::new(
                    "GSU-E002",
                    "Explicit configuration must contain [tool.gsu]",
                    Some(relative(&path, &root)),
                )
            })?;
        } else {
            for parent in cwd.ancestors() {
                let path = parent.join("pyproject.toml");
                if path.exists()
                    && let Some(found) = read(&path, cwd)?
                {
                    settings = found;
                    root = parent.to_path_buf();
                    break;
                }
                if parent.join(".git").exists() {
                    break;
                }
            }
        }
        let preview = if args.no_preview {
            false
        } else {
            args.preview || settings.preview.unwrap_or(false)
        };
        let selected = expand(
            args.select
                .as_deref()
                .map(cli_list)
                .or(settings.select)
                .unwrap_or_else(|| {
                    RULES
                        .iter()
                        .filter(|r| preview || !r.preview)
                        .map(|r| r.code.to_string())
                        .collect()
                }),
            preview,
            true,
        )?;
        let ignored = expand(
            args.ignore
                .as_deref()
                .map(cli_list)
                .or(settings.ignore)
                .unwrap_or_default(),
            preview,
            false,
        )?;
        let patterns = if args.exclude.is_empty() {
            settings.exclude.unwrap_or_default()
        } else {
            args.exclude.clone()
        };
        let mut builder = GlobSetBuilder::new();
        let mut components = vec![];
        for pattern in patterns {
            let pattern = pattern.replace('\\', "/");
            if pattern.starts_with('!') || pattern.contains(['{', '}']) {
                return Err(OperationError::new(
                    "GSU-E002",
                    format!("Unsupported exclude pattern: {pattern}"),
                    None,
                ));
            }
            if !pattern.contains(['/', '*', '?', '[', ']']) {
                components.push(pattern.clone());
            }
            builder.add(
                GlobBuilder::new(&pattern)
                    .literal_separator(true)
                    .build()
                    .map_err(|e| OperationError::new("GSU-E002", e.to_string(), None))?,
            );
        }
        Ok(Self {
            root,
            enabled: selected.difference(&ignored).cloned().collect(),
            excludes: builder
                .build()
                .map_err(|e| OperationError::new("GSU-E002", e.to_string(), None))?,
            components,
        })
    }
    pub fn excluded(&self, path: &Path) -> bool {
        let rel = relative(path, &self.root);
        rel.split('/')
            .any(|s| self.components.iter().any(|c| c == s))
            || Path::new(&rel)
                .ancestors()
                .any(|p| self.excludes.is_match(p))
    }
}
