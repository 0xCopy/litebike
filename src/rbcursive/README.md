# rbcursive

RBCursive is Litebike's SIMD-accelerated parser-combinator subsystem. This folder contains the parsing primitives, platform-specific SIMD backends, and small protocol detectors used across Litebike.

Policy:
- Non-bounty, project-specific design notes have been consolidated under `docs/betanet-densifier.md`.
- Micro-protocol authors should implement minimal `MicroProtocol` types and publish them as small crates that depend only on `rbcursive`'s public API.

Micro-protocol guidance:
- Keep dependencies minimal. Prefer `no_std`-compatible crates where possible.
- Expose a single type implementing `MicroProtocol` and register it with higher-level code using composition rather than broad feature flags.
- Avoid transitive heavy deps; use bounded-size data structures and explicit lifetimes.
