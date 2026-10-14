# Card: hgl-kernel-prepared-keyed

The `prepared_keyed` module uses real `nested::PreparedChild<T>` instances,
normal child scopes and indexed deadlines. Public `PreparedKeyed<T>::new(pools)`
accepts exact membership tokens with cold independent fresh-instance pools.
`insert(key,ctx,bind)` starts a fresh membership lifetime once, binding the
retained boundary immediately before start, or preserves an existing lifetime.
`remove(key,ctx)` stops and retires the active scope through the ordinary runtime
protocol. `evaluate(ctx)` drives only notified or due live children and propagates
uncaught errors. `stop(ctx)` stops active children in reverse creation order and
discards unused prepared scopes, preserving the first failure.
`refresh(key,ctx,bind)` updates a live member's boundary in the parent's scope,
retaining its graph state and routing new child notifications normally.
`discard_captured_pass(ctx)` recursively abandons the due pass in active children
when an enclosing explicit capture consumes a failure, then resynchronizes the
keyed heap with future deadlines. This failure-only scan preserves live state and
scopes, and never resumes the abandoned pass as a retry.

Key and scope maps, activation order and indexed deadlines are reserved cold.
Notifications from removed scopes cannot schedule recreated children. Pools are
never recycled into a stopped graph; an unprepared key or exhausted activation
bound reports a target capacity failure. The module does not implicitly capture
evaluation failures and does not create a secondary endpoint retirement lifetime.
