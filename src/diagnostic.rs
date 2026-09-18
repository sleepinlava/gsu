use serde::Serialize;
#[derive(Clone, Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct Location {
    pub path: String,
    pub start: Position,
    pub end: Position,
}
#[derive(Clone, Debug, Serialize)]
pub struct Evidence {
    pub kind: String,
    pub detail: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic {
    pub rule: &'static str,
    pub severity: &'static str,
    pub confidence: &'static str,
    pub location: Location,
    pub message: &'static str,
    pub explanation: &'static str,
    pub suggestion: &'static str,
    pub evidence: Vec<Evidence>,
    #[serde(skip)]
    pub excerpt: String,
    #[serde(skip)]
    pub underline: String,
}
#[derive(Debug, Serialize)]
pub struct OperationError {
    pub code: &'static str,
    pub message: String,
    pub path: Option<String>,
}
impl OperationError {
    pub fn new(code: &'static str, message: impl Into<String>, path: Option<String>) -> Self {
        Self {
            code,
            message: message.into(),
            path,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct Skipped {
    pub path: String,
    pub reason: &'static str,
}
#[derive(Default, Debug, Serialize)]
pub struct Summary {
    pub files_discovered: usize,
    pub files_checked: usize,
    pub files_failed: usize,
    pub files_skipped: usize,
    pub diagnostics: usize,
}
#[derive(Debug, Serialize)]
pub struct RunReport {
    pub schema_version: &'static str,
    pub tool_version: &'static str,
    pub complete: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub errors: Vec<OperationError>,
    pub skipped: Vec<Skipped>,
    pub summary: Summary,
}
impl Default for RunReport {
    fn default() -> Self {
        Self {
            schema_version: "2",
            tool_version: env!("CARGO_PKG_VERSION"),
            complete: true,
            diagnostics: vec![],
            errors: vec![],
            skipped: vec![],
            summary: Summary::default(),
        }
    }
}
impl RunReport {
    pub fn finish(&mut self) {
        self.diagnostics.sort_by(|a, b| {
            (&a.location.path, &a.location.start, a.rule, &a.location.end).cmp(&(
                &b.location.path,
                &b.location.start,
                b.rule,
                &b.location.end,
            ))
        });
        self.diagnostics.dedup_by(|a, b| {
            a.rule == b.rule
                && a.location.path == b.location.path
                && a.location.start == b.location.start
                && a.location.end == b.location.end
        });
        self.errors
            .sort_by(|a, b| (&a.path, a.code, &a.message).cmp(&(&b.path, b.code, &b.message)));
        self.skipped.sort_by(|a, b| a.path.cmp(&b.path));
        self.summary.diagnostics = self.diagnostics.len();
        self.summary.files_skipped = self.skipped.len();
        self.complete = self.errors.is_empty();
    }
    pub fn exit_code(&self) -> u8 {
        if !self.complete {
            2
        } else {
            u8::from(!self.diagnostics.is_empty())
        }
    }
}
