
## Example 2

```rust
use example_native_interface::Native;

pub struct Implementation;

impl Native for Implementation {
    fn bit_and(lhs: i64, rhs: i64) -> i64 {
        lhs & rhs
    }
}
```


## Example 4

```rust
fn accumulate(value: Input<'_, i64>, out: Output<'_, i64>, logger: Logger<'_>);
fn describe(value: i64, logger: Logger<'_>) -> String;
```
