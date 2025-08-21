# Litebike Fuzzer

Small extensible client->host fuzzer for the Litebike project.

Usage:

```sh
# build
cargo build --manifest-path tools/fuzzer/Cargo.toml

# run against a local mock target
cargo run --manifest-path tools/fuzzer/Cargo.toml -- --target 127.0.0.1:9000 --iterations 100
```

Development:
- Unit tests are embedded in `src/main.rs` and can be run via:

```sh
cargo test --manifest-path tools/fuzzer/Cargo.toml
```

Extending:
- Replace `mutate` with more advanced mutators or integrate a plugin system.
- Add a corpus directory named `seeds/` with files to exercise parsers.
