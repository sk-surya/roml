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

<…other sections can go here…>