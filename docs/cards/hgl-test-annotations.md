# hgl-test-annotations

Status: accepted

Lexical expectation metadata for ordinary mixed HGL test runs. annotations reads
actual standalone line comments and returns Expectation records with the exact
following line, category and catalogue code. Empty metadata is valid. Strings
and block comments are ignored, malformed annotations fail admission. May use
hgl-source and hgl-diagnostics; never classify compiler errors by message.
