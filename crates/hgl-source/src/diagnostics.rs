//! Source-origin diagnostic identity and rendering, independent of message wording.
use std::ops::Range;

/// A source-checking failure before its owning file is attached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// Original owning file when a failure crosses a declaration boundary.
    pub source: Option<String>,
    /// Normative diagnostic category.
    pub category: &'static str,
    /// Stable identity, absent for uncatalogued failures.
    pub code: Option<&'static str>,
    /// Primary byte range in the original source.
    pub span: Range<usize>,
    /// Human-readable context, never used for matching.
    pub message: String,
}
impl Issue {
    /// Reject a resolved source shape without changing its exact identity.
    pub fn delta_shape(name: &str) -> Self {
        let mut issue = Self::from(format!("delta: unsupported publication shape {name}"));
        issue.category = "shape";
        issue.code = Some("delta.unsupported_shape");
        issue
    }
    /// Retain an uncatalogued ordinary type failure for contextual diagnostics.
    pub fn typed(span: Range<usize>, message: impl Into<String>) -> Self {
        let mut issue = Self::from(message.into()).at(span);
        issue.category = "type";
        issue
    }
    /// Construct a coded failure at the rule's primary source location.
    pub fn coded(
        category: &'static str,
        code: &'static str,
        span: Range<usize>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            source: None,
            category,
            code: Some(code),
            span,
            message: message.into(),
        }
    }
    /// Attach a source location to an otherwise unlocated failure.
    #[must_use]
    pub fn at(mut self, span: Range<usize>) -> Self {
        if self.span == (0..0) {
            self.span = span;
        }
        self
    }
    /// Retain the first rejected candidate's primary identity with the full context.
    pub fn alternatives(errors: Vec<Self>, context: String) -> Self {
        let reasons = errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        let mut primary = errors
            .into_iter()
            .next()
            .unwrap_or_else(|| context.clone().into());
        primary.message = if reasons.is_empty() {
            context
        } else {
            format!("{context}: {reasons}")
        };
        primary
    }
    /// Attach owning source without replacing an earlier diagnostic origin.
    #[must_use]
    pub fn in_source(mut self, source: &str) -> Self {
        self.source.get_or_insert_with(|| source.into());
        self
    }
    /// Translate a relative argument range into its containing source range.
    #[must_use]
    pub fn shifted(mut self, offset: usize) -> Self {
        if self.source.is_some() {
            return self;
        }
        self.span.start += offset;
        self.span.end += offset;
        self
    }
}
impl From<String> for Issue {
    fn from(message: String) -> Self {
        Self {
            source: None,
            category: "parse",
            code: None,
            span: 0..0,
            message,
        }
    }
}
impl From<&str> for Issue {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}
impl std::fmt::Display for Issue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(source) = &self.source {
            write!(f, "{source}:{}: ", self.span.start)?;
        }
        write!(f, "{}", self.category)?;
        if let Some(code) = self.code {
            write!(f, "[{code}]")?;
        }
        write!(f, ": {}", self.message)
    }
}
impl From<Issue> for String {
    fn from(issue: Issue) -> Self {
        issue.to_string()
    }
}
/// A primary error tied to its exact source file and source line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Source filename, compared exactly by fixture matching.
    pub source: String,
    /// One-based physical line of the primary range start.
    pub line: usize,
    /// One-based Unicode-character column of the primary range start.
    pub column: usize,
    /// Structured failure and original byte range.
    pub issue: Issue,
}
impl Diagnostic {
    /// Resolve a primary byte range against the original source text.
    pub fn new(source: &str, text: &str, issue: Issue) -> Self {
        let offset = issue.span.start.min(text.len());
        Self {
            source: source.into(),
            line: text[..offset].split('\n').count(),
            column: text[..offset]
                .rsplit('\n')
                .next()
                .map_or(1, |line| line.chars().count() + 1),
            issue,
        }
    }
}
impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.source, self.line, self.column, self.issue.category
        )?;
        if let Some(code) = self.issue.code {
            write!(f, "[{code}]")?;
        }
        write!(f, ": {}", self.issue.message)
    }
}
/// Normative diagnostic categories, including infrastructure-only build errors.
pub const CATEGORIES: &[&str] = &[
    "parse",
    "name",
    "type",
    "shape",
    "constraint",
    "function-kind",
    "phase",
    "injectable",
    "operator",
    "module",
    "build",
];
/// Initial source-error catalogue: exact category and code pairs.
pub const SOURCE_CODES: &[(&str, &str)] = &[
    ("parse", "syntax.expected_token"),
    ("type", "rolling.size_kind"),
    ("type", "rolling.size_bounds"),
    ("type", "yield.time_type"),
    ("type", "test.raises_code"),
    ("phase", "test.statement_phase"),
    ("shape", "delta.unsupported_shape"),
    ("type", "delta.type_mismatch"),
    ("name", "delta.argument_name"),
    ("name", "delta.duplicate_argument"),
    ("type", "delta.entry_constant"),
    ("type", "delta.entry_type"),
    ("type", "delta.duplicate_entry"),
    ("type", "delta.index_bounds"),
    ("type", "delta.overlap"),
];
/// Initial execution-error catalogue, disjoint from source-error codes.
pub const EXECUTION_CODES: &[&str] = &[
    "yield.negative_duration",
    "yield.non_increasing_time",
    "eval.input_delta_profile",
];

/// Render a typed failure against its original source, retaining physical lines.
pub fn render_issue(sources: &[(String, String)], issue: Issue) -> String {
    let source = issue.source.clone().unwrap_or_else(|| {
        sources
            .first()
            .map_or_else(String::new, |(name, _)| name.clone())
    });
    let text = sources
        .iter()
        .find(|(name, _)| *name == source)
        .map_or("", |(_, text)| text.as_str());
    Diagnostic::new(&source, text, issue).to_string()
}
/// Return every rendered primary source error without changing its identity.
pub fn ensure(errors: Vec<Diagnostic>) -> Result<(), String> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("\n"))
    }
}
