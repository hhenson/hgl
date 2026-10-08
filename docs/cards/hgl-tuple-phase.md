# Card: hgl-tuple-phase

Check Tuple construction phase at constant source boundaries before runtime
specialization. The tuple_phase module has an initial 100 source-line budget
within the unchanged hgl-semantics limit; it uses Library, name_check visibility and struct_names.

Public surface: `depends`, `constant`, `call`, `signature`.
Known temporal parameters, their ordinary aliases and assignments, state/cache bindings, and injected clock/out/global_state payloads
retain runtime provenance in lexical tuple_flow Facts. This does not change the ordinary
category of local values. A Tuple literal, nested tuple or retained alias depending on that provenance cannot satisfy an
unambiguously constant formal or constant default. Overload ambiguity remains
with existing typed resolution. Runtime tuple construction elsewhere is admitted.
Source failures use ordinary uncatalogued type diagnostics, with the Tuple's
source span; no delta error code is substituted.

Acceptance: both published tuple constant-context rejection fixtures, unused
clock/property and temporal-alias controls, runtime constructors and independent
ordinary Tuple children in the positive example.
