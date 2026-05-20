# ROML

A production‑grade, incremental MILP modeling library implemented in Rust.

## Logging

The crate uses the `log` facade for all logging calls.  At runtime you can
configure the logger using [log4rs](https://docs.rs/log4rs) and a YAML file.
A sample configuration file (`log4rs.yaml`) is included at the project root
which writes messages to both `stdout` and a file named `roml.log`.

Two environment variables are supported:

* `LOG4RS_CONFIG` – if set, the given path is used as the configuration file
  instead of searching upwards for `log4rs.yaml`.
* `ROML_LOG_FILE` – used by the example configuration to determine where the
  `logfile` appender writes.  If unset `init_logging()` will attempt to locate
  the Cargo workspace root (a `Cargo.toml` containing `[workspace]`) and set
  this variable to `<root>/roml.log` automatically.  This makes it easy to run
  tests from any crate directory and still end up with a single log in the
  workspace root.

Call `roml::init_logging()` early in your application (e.g. from `main`) to
load the configuration; the function handles the environment logic above and
returns an error if the configuration itself cannot be parsed.

Example:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // initialise logger before doing anything else; will set ROML_LOG_FILE
    // automatically if you're inside a workspace.
    roml::init_logging()?;

    let mut model = roml::Model::new();
    // ...
    Ok(())
}
```

If the configuration file cannot be found or parsed, `init_logging` returns an
error which your application can handle according to its policy (tests may
ignore it).

## Usage

ROML now exposes three complementary modeling layers:

- Explicit low-level APIs such as `add_constraint`, `add_coeff`, and `add_objective_coefficient`
- Fluent builder APIs such as `.le(...)`, `.between(...)`, `.maximize()` and `Model::constrain(...)`
- Optional macros for math-like call sites such as `constraint!(x + y <= 4.0)` and effectful wrappers such as `constrain!(model, x + y <= 4.0)`

Typical end-to-end usage with the HiGHS adapter looks like this:

```rust
use roml::prelude::*;
use roml::{constrain, constraint, objective, set_objective};
use roml_highs::HighsAdapter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut model = Model::new();

  let x = model.add_var();
  let y = model.add_var();
  let price = model.add_parameter(1.0);

  constrain!(model, x + y <= 4.0)?;
  constrain!(model, x <= 3.0)?;
  constrain!(model, between: 0.0, y, 3.0)?;

  let obj = set_objective!(model, maximize: price * x + y + 2.0)?;

  model.set_parameter(price, 3.0);
  model.commit();

  let mut adapter = HighsAdapter::new();
  let solution = adapter.solve_model(&mut model)?;

  assert_eq!(model.objective_constant(obj), Some(2.0));
  assert!(solution.is_optimal());
  Ok(())
}
```

If you prefer the method-based API, the same constraint and objective setup can be written without macros:

```rust
use roml::prelude::*;

let mut model = Model::new();
let x = model.add_var();
let y = model.add_var();

model.constrain((x + y).le(4.0))?;
let obj = model.maximize(x + 2.0 * y + 5.0)?;
assert_eq!(model.objective_constant(obj), Some(5.0));
# Ok::<(), roml::ModelError>(())
```

## Parameters And Transactions

Parameter updates are intentionally queued.

- `set_parameter` records pending changes in the current transaction.
- `commit` applies the queued parameter values and propagates them to dependent coefficients as one batch.
- `drain_changes` will auto-commit pending parameter updates and emit a warning if you forgot to commit explicitly.

That behavior is deliberate: it keeps bulk parameter updates explicit today and leaves room for future transaction-level optimization work.