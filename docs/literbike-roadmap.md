# Literbike roadmap: fork and migration checklist

This document captures the minimal, actionable steps to fork `litebike` into `literbike` and co-evolve the non-bounty specs into a cleaner, production-ready lineage.

### Lineage note

The following projects are evolutions of the `RelaxFactory` REST reactor (see `../RelaxFactory/`): `../couchduckdb`, `../couchdbcascade`, and the betanet couchduck variants. Preserve design notes from RelaxFactory when migrating.

## Goals

- Keep the core crate small, auditable, and well-tested.
- Move bounty-specific or experimental code to `tools/bounties/` or separate repos.
- Provide stable public APIs for cursor abstractions and persistence adapters.

## Migration checklist

1. Code triage
   - Audit `src/` and `tools/` for bounty-specific code paths.
   - Move or copy bounty code into `tools/bounties/<name>/` with a short README per bounty.

2. Crate and metadata rename
   - Update `Cargo.toml` crate name (e.g., `name = "literbike"`) and check for other references in docs and CI.
   - Preserve git history; create a branch `literbike-init` before renaming for an easy revert.

3. Public API: cursor and persister
   - Define `src/cursor.rs` with a minimal, documented trait and token serialization.
   - Add `src/persister.rs` trait for token persistence with at least two implementations: `persister::couchdb` and `persister::file`.

4. Backends/adapters
   - Create `src/backends/couchdb.rs` and `src/backends/ipfs.rs` implementing the cursor trait.
   - Ensure token shapes are JSON-serializable and include `backend` fields.

5. Tests and CI
   - Add unit tests for token serialization, seek/resume semantics, and a mock backend.
   - Add integration tests (optional in CI) using docker-compose for CouchDB and local IPFS.

6. Docs and migration guide
   - Add `docs/cursor.md` describing the cursor contract and examples.
   - Add migration notes in `docs/migration.md` describing breaking changes and upgrade paths.

7. Release and tagging
   - Cut an initial `v0.1.0` release for `literbike` after the above are green.

# Minimal commands and checks

## create a branch and run tests

```bash
git checkout -b literbike-init
cargo test
```

# update crate name in Cargo.toml, run quick build

```bash
# edit Cargo.toml
cargo build --release
```

## Next steps I can take

- Implement `src/cursor.rs` with trait + JSON token type and add unit tests.
- Add a small in-memory mock backend and CI tests for seek/resume.
- Implement the CouchDB adapter with seq persistence into a `_cursor/meta/<consumer>` doc.

If you want, I can start by implementing `src/cursor.rs` and a mock backend with tests; tell me which adapter to prioritize after that (CouchDB or IPFS).
