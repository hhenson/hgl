//! Owned HGL documentation and reST export, independent of compiler backends.

/// An attached Google-style/reST document, retained independently of generated code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Documentation {
    /// Qualified HGL declaration name, not a generated Rust name.
    pub name: String,
    /// Original declaration signature, without an executable body.
    pub declaration: String,
    /// Normalized reST content with Google-style sections.
    pub text: String,
    /// Selected source part; empty for the shared declaration file.
    pub part: String,
    /// Original source name for diagnostics and editor navigation.
    pub source: String,
    /// Documentation's starting byte offset.
    pub start: usize,
    /// Documentation's exclusive ending byte offset.
    pub end: usize,
}
/// Remove delimiters and common indentation, preserving relative reST layout.
/// The caller supplies a complete documentation comment of at least five bytes.
pub fn normalize(raw: &str) -> String {
    let mut lines: Vec<_> = raw[3..raw.len() - 2].lines().map(str::to_owned).collect();
    if let Some(first) = lines.first_mut() {
        *first = first.trim_matches([' ', '\t']).into();
    }
    let indent = lines
        .iter()
        .skip(1)
        .filter(|line| !line.trim_matches([' ', '\t']).is_empty())
        .map(|line| line.len() - line.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    for line in lines.iter_mut().skip(1) {
        *line = line.get(indent..).unwrap_or("").into();
    }
    while lines
        .first()
        .is_some_and(|line| line.trim_matches([' ', '\t']).is_empty())
    {
        lines.remove(0);
    }
    while lines
        .last()
        .is_some_and(|line| line.trim_matches([' ', '\t']).is_empty())
    {
        lines.pop();
    }
    lines.join("\n")
}
fn heading(line: &str) -> bool {
    matches!(
        line,
        "Args:"
            | "Type Args:"
            | "Returns:"
            | "Raises:"
            | "Ticks:"
            | "Validity:"
            | "Properties:"
            | "Requires:"
            | "Notes:"
            | "Examples:"
    )
}
/// Validate parameter keys for a nongeneric declaration without properties or requirements.
pub fn validate(text: &str, parameters: &[&str]) -> Result<(), String> {
    let mut section = "";
    for line in text.lines() {
        if heading(line) {
            section = line;
            continue;
        }
        if !line.is_empty() && !line.starts_with(' ') {
            section = "";
        }
        let Some(content) = line.strip_prefix("    ").filter(|s| !s.starts_with(' ')) else {
            continue;
        };
        let Some((key, _)) = content.split_once(':') else {
            continue;
        };
        let key = key.trim_matches([' ', '\t']);
        let valid = match section {
            "Args:" => parameters.contains(&key),
            "Type Args:" | "Properties:" | "Requires:" => false,
            _ => true,
        };
        if !valid {
            return Err(format!("documentation {section} unknown key '{key}'"));
        }
    }
    Ok(())
}
/// Emit reStructuredText suitable for Sphinx with math and Mermaid extensions.
/// This does not execute directives or interpret documentation as Markdown.
pub fn render(documents: &[Documentation]) -> String {
    let mut out = Vec::new();
    for doc in documents {
        let title = if doc.part.is_empty() {
            doc.name.clone()
        } else {
            format!("{} ({})", doc.name, doc.part)
        };
        out.push(format!(
            "{title}\n{}\n\n.. code-block:: text\n\n",
            "-".repeat(title.len())
        ));
        for line in doc.declaration.lines() {
            out.push(format!("    {line}\n"));
        }
        out.push("\n".into());
        let mut section = "";
        for line in doc.text.lines() {
            if heading(line) {
                out.push(format!("\n.. rubric:: {}\n\n", &line[..line.len() - 1]));
                section = line;
            } else {
                if !line.is_empty() && !line.starts_with(' ') {
                    section = "";
                }
                let line = if section.is_empty() {
                    line
                } else {
                    line.strip_prefix("    ").unwrap_or(line)
                };
                if matches!(section, "Args:" | "Type Args:")
                    && !line.starts_with(' ')
                    && let Some((name, description)) = line.split_once(':')
                {
                    out.push(format!(
                        "``{name}``\n    {}\n",
                        description.trim_matches([' ', '\t'])
                    ));
                    continue;
                }
                if matches!(section, "Properties:" | "Requires:") && line.ends_with(':') {
                    out.push(line[..line.len() - 1].into());
                } else {
                    out.push(line.into());
                }
                out.push("\n".into());
            }
        }
        out.push("\n".into());
    }
    out.concat()
}
