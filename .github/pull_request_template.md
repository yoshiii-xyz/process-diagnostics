## Scope

- [ ] The change stays within exact process diagnostics and bounded transcripts.
- [ ] No implicit shell, task runner, pipeline, hosted service, or untested
      platform claim was added.
- [ ] No private environment values or generated fuzz corpus files are included.

## Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --all-targets --locked`
- [ ] `cargo clippy --all-targets --all-features --locked -- -D warnings`
- [ ] `cargo test --all-targets --locked`
- [ ] `RUSTDOCFLAGS=-Dwarnings cargo doc --no-deps --locked`
- [ ] `cargo package --locked`
- [ ] `cargo audit`
