# Card: hgl-diagnostics

Source-origin diagnostic identity shared by parsers, checkers and rejection
fixtures. No dependencies; budget 200 source lines. Public Issue contains
category, optional code, original byte span and message; coded constructs a
catalogued rule failure, at supplies missing location. From<String> preserves
uncoded parser failures; conversion back to String serves legacy callers.
Diagnostic contains exact source filename, primary physical line and Issue;
new computes line from the original text and Display renders human context.
CATEGORIES, SOURCE_CODES and EXECUTION_CODES enumerate the normative initial
catalogue. Matching never inspects diagnostic message text.

Issue.source optionally preserves an owning filename across resolution. in_source
sets it only when absent; shifted translates relative ranges. Display and String
conversion retain category/code/origin rather than reducing a failure to its
message. Typed APIs consume Issue directly, never parse that display text.

Issue::alternatives retains the first rejected candidate's primary identity and
adds candidate context to its message; no diagnostic is reconstructed from text.
Empty candidate sets produce an uncoded context issue.

render_issue resolves a source-bearing Issue to its original physical line and
renders it. ensure returns all rendered diagnostics as an ordinary Result;
these are terminal presentation boundaries, never source classification.

Diagnostic.column records the one-based character column; Display preserves the
ordinary file:line:column location form. Rejection matching still ignores columns.
