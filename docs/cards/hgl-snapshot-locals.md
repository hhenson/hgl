# Card: hgl-snapshot-locals

Cold destination planning for checked complete ordinary observations.
Public `prepare(&mut Plan, storage: impl Fn(&Ty) -> Ty)` reserves private typed
snapshot globals for lexical owners, including nested ordinary helper bodies.
Callsite IDs are normalized before this pass. Synthesized return/assignment
owners use IDs above all existing declarations. The callback supplies backend
storage types; this module performs no emission or expression evaluation.
May use ir, ir_visit and call_locals for shared declaration-ID scanning; budget 150 within the unchanged semantics crate cap.

Acceptance: generated observation and Any ownership tests preserve capture
isolation, lexical identity and allocation-free finite evaluation.

The declaration maximum covers start, stop, handlers and generator bodies before
adding synthesized owners; nested helper maxima are included after callsite
normalization. Generator declarations cannot be overwritten by scratch owners,
including when an invoked helper has no local declarations.

Public prepare_with extends prepare and receives the checked prepared-execution flag. Native snapshot result, assignment and global-write destinations use the same lexical reservation policy; mutable owners retain their complete independently prepared representation.
