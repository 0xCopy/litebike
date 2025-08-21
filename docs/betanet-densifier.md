# Betanet Densifier (moved from internal agent note)

This document captures the densifier axioms and guidance for Betanet-related work. It is intentionally placed in `docs/` so it is discoverable and not mixed with `rbcursive`'s micro-protocol guidance.

(Original content preserved from internal draft; maintainers may trim bounty-specific items before public release.)

## Summary
- Focus: zero-cost abstractions, SIMD anchors, io_uring/eBPF integration, and FSM-based bounty tracking.
- Place bounty-specific implementations in `tools/bounties/` or separate crates to keep core code lean.

Note: `litebike` is an afero library curated by the maintainers; `betanet 1.1` is treated as a bounty and its implementations should live in `tools/bounties/` or separate repos.

(For full details, see the internal agent note.)

## Co-evolving non-bounty specs into litebike

This repository will keep focusing on the core, low-level networking and protocol primitives (zero-cost abstractions, SIMD anchors, io_uring/eBPF integration, and FSM-based tracking). Bounty-specific implementations and ephemeral experiments should live separately under `tools/bounties/` or in dedicated crates to keep the core codebase lean and reviewable.

When we say "co-evolve non-bounty specs into litebike" we mean:

- Extract and stabilize interface-level specs (protocol contracts, cursor/cursor-manager, persistence adapters) that are broadly useful beyond any one bounty.
- Move ephemeral or revenue-driven implementations to `tools/` or a sibling repository so the core crate remains small and auditable.
- Add minimal, well-tested adapter layers that let the same code run against CouchDB (kv), IPFS, or other backends without duplicating logic.

### Forking plan: create `literbike` from here

We will fork from this base to start the `literbike` lineage. The fork will:

1. Remove or relocate bounty-specific code to `tools/bounties/` (or a separate repo).
2. Rename repository metadata where needed (crate name, docs references) while preserving git history.
3. Harden and document the cursor abstraction and persister APIs as public, stable surfaces.
4. Add migration guides and backwards-compatibility notes for integrators.

This document is a lightweight, discoverable pointer for maintainers. See `docs/literbike-roadmap.md` for a concrete migration checklist and next steps.

### Lineage

The following projects are evolutions of the `RelaxFactory` REST reactor located at `../RelaxFactory/`:

- `../couchduckdb`
- `../couchdbcascade`
- `betanet`'s couchduck integrations

These implementations adapt the RelaxFactory reactor model into specialized persistence and replication layers. When migrating or forking, preserve the relevant history and design notes from `RelaxFactory`.
