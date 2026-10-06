# hgl-reject

Status: accepted

Mixed source-rejection orchestration; budget 250 source lines. No build or graph
execution occurs here. Outcomes match every originating primary diagnostic once
by exact source, next physical line, category and stable code.

Plan::prepare assigns explicit-file annotations to safely bounded owners and masks every rejection owner preserving source locations. Imported metadata is ignored. select validates names against executable and rejection cases. check restores each selected owner independently and returns Outcome records with exact diagnostic matching. selected implements ordinary short/qualified name matching. May use hgl-test-annotations, hgl-test-units, hgl-program and hgl-diagnostics. Module cases always run; all rejected declarations are absent from other probes.
