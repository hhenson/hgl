# Card: hgl-reject

Compile-rejection fixture runner; dependencies diagnostics, source, program;
budget 250 source lines. Public reject(&Path)->Result<(),String> reads one file,
validates actual standalone line-comment expectations with lexical string/block
comment handling, runs the same structural and actual semantic validation as ordinary
checking, and matches every primary error once by exact file, next physical
line, category and stable code. Unknown/malformed annotations, missing/extra
errors and successful source checking fail. Infrastructure failures are labelled
and cannot satisfy expectations. Does not build artifacts or execute graphs.
