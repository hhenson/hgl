# Card: hgl-owned-columns

The private owned_columns module shares reusable buffer allocation, reclamation and disjoint-slot copying between Any and native scalar storage. Its budget is 80 lines within the unchanged store cap; it has no internal module dependencies.

Reservation grows storage and the free-position list before installation. Typed callers supply the infallible copy operation after whole-value preflight. Identity-slot copies do nothing. Slots remain stable and no hot capacity growth or type inspection is introduced.
