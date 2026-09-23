#[cfg(feature = "codespan-reporting")]
use crate::model::Issue;
#[cfg(feature = "rowan")]
use crate::model::Token;

#[cfg(feature = "rowan")]
pub fn lossless(source: &str, tokens: &[Token]) -> String {
    let mut builder = rowan::GreenNodeBuilder::new();
    builder.start_node(rowan::SyntaxKind(100));
    for token in tokens {
        builder.token(
            rowan::SyntaxKind(token.kind as u16),
            &source[token.span.clone()],
        );
    }
    builder.finish_node();
    builder.finish().to_string()
}
#[cfg(feature = "codespan-reporting")]
pub fn diagnostics(source: &str, issues: &[Issue]) -> String {
    use codespan_reporting::{
        diagnostic::{Diagnostic, Label},
        files::SimpleFiles,
        term::{self, Config},
    };
    let mut files = SimpleFiles::new();
    let file = files.add("case.hgl", source);
    let mut output = Vec::new();
    for issue in issues {
        let d = Diagnostic::error()
            .with_code("syntax")
            .with_message(&issue.message)
            .with_labels(vec![Label::primary(file, issue.span.clone())]);
        term::emit_to_io_write(&mut output, &Config::default(), &files, &d).unwrap();
    }
    String::from_utf8(output).unwrap()
}
