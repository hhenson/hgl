# Card: hgl-execution-phase

Node lifecycle metadata and the generic executed-host context protocol shared by
frontend checks and cold execution. Module budget 40 lines within the unchanged
Source aggregate; no provider implementation dependency.

Public ExecutionPhase is Start, Evaluation (default), or Stop. name returns the
exact descriptor phase spelling during cold checking. This moves the existing
frontend phase record without changing endpoint authority or executing providers.

ExecutionContext<V,N,E> supplies temporal recipes and optional native execution.
Temporal-only callbacks implement the protocol with native execution absent;
checking therefore cannot activate provider code. The target's emitted host adds
typed callbacks only for fully checked native bindings during executed setup or
cold materialization.
