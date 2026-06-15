# zm-mux

Cross-platform (Windows-without-WSL + macOS) AI-agent terminal multiplexer — a
clean-room, native-Rust answer to the macOS-only **cmux**.

> **Status: research phase.** This repository currently holds a *precise,
> source-verified investigation* (no implementation yet). See
> [`docs/research/`](docs/research/00-overview.md). The scaffold + MVP land in a
> later, separately-approved plan.

## Goal (requirements)

1. A program usable on **Windows** that is *nearly equivalent* to cmux (cmux is macOS-only).
2. **Cross-platform** (Windows + macOS, Linux as a bonus).
3. Runs on Windows **without WSL** (native ConPTY).

## What's here

| Path | Contents |
|------|----------|
| [`docs/research/`](docs/research/00-overview.md) | The investigation: cmux analysis, Windows/no-WSL feasibility, target architecture, AI-agent integration, reference inventory, roadmap. Every claim tagged **[V]** verified / **[I]** inferred / **[?]** unverified, with sources. |
| `scripts/clone-references.{sh,ps1}` | Reproduce the local reference set (cloned into `reference/`, which is gitignored). |
| `reference/` *(gitignored)* | Cloned upstream repos used as **read-only** study material. Inventory + pinned SHAs in [`docs/research/05-reference-inventory.md`](docs/research/05-reference-inventory.md). |

## Licensing intent

zm-mux is **clean-room MIT/Apache**. cmux is dual-licensed **GPL-3.0-or-later OR
commercial**; it (and other copyleft references) are studied for *understanding
only* — no code/text is copied into zm-mux. Implementation is built from
permissively-licensed Rust crates. See
[`docs/research/05-reference-inventory.md`](docs/research/05-reference-inventory.md).
