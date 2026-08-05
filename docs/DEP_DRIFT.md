# dep-drift (deep)

Compare **dependency intent** (Cargo.toml) and optionally **resolved versions**
(Cargo.lock) between two trees — monorepo vs split, fork vs upstream, etc.

```sh
cargo-tog dep-drift --master /path/to/main --other /path/to/split
cargo-tog dep-drift --master . --other ../cli --json
cargo-tog dep-drift --master . --other ../cli --ignore serde,serde_json
```

## Why the naive check was wrong

A shallow scan that only compared **quoted** versions and **skipped**
`workspace = true` reported “clean” for:

| Tree | Deps |
|------|------|
| Monorepo | `tokio` pin in `[workspace.dependencies]`, members use `workspace = true` |
| Split CLI | `tokio.workspace = true` **without** a local workspace table |

There was **no overlapping explicit string** → false **OK**. The split cannot
even resolve `tokio` alone.

## What it checks now

### 1. Manifest requirements (always)

For every `Cargo.toml` under each root (skipping `target/`, `.git`, …):

| Form | Resolution |
|------|------------|
| `foo = "1"` | explicit `1` |
| `foo = { version = "1", … }` | explicit `1` |
| `foo = { workspace = true }` / `foo.workspace = true` | look up `[workspace.dependencies]` in **that tree** |
| pin found | `workspace→1` |
| pin missing | `workspace?(unpinned)` ← **failure** |
| `path = "…"` | path dep (ignored in drift keys unless `--include-path`) |
| `git = "…"` | git dep |

Then:

- **requirement drift** — same crate name, different resolved keys  
- **unresolved workspace on other/master** — broken standalone pins  
- **only in master / only in other** — presence (informational; use flags to fail)

### 2. Lockfile exact versions (default when both have `Cargo.lock`)

For crates that are **direct deps** on either side:

- Compare sorted version lists from each lock  
- Catches “req still `1` but lock jumped `1.38` → `1.40`” between trees  

Disable with `--no-lock`.

## Exit code

| Situation | Default exit |
|-----------|----------------|
| req drift, lock drift, or unpinned workspace on **other** | **1** |
| only-in-master / only-in-other | **0** (use `--fail-missing` / `--fail-extra`) |
| clean | **0** |
| `--warn-only` | always **0** |

## Flags

| Flag | Meaning |
|------|---------|
| `--json` | Full machine report |
| `--no-lock` | Skip lock comparison (otherwise on when both locks exist) |
| `--ignore a,b` | Skip crates |
| `--include-path` | Don’t ignore path deps |
| `--show-ok` | List matching crates |
| `--fail-extra` | Exit 1 if other has extra crates |
| `--fail-missing` | Exit 1 if other lacks master’s non-path crates |
| `--warn-only` | Report but exit 0 |

## Interpreting monorepo vs partial CLI mirror

Typical **healthy** partial mirror while still monorepo-built:

- Unpinned `workspace?` on the split for crates that only resolve in monorepo  
- That **should fail** if you claim the split is standalone  
- If the split is **only** a publish surface and always builds from monorepo, either:
  - keep failing dep-drift as a reminder, or  
  - `--ignore` those crates / don’t run drift until standalone, or  
  - copy `[workspace.dependencies]` pins into the split  

Exact version agreement when both have locks = strongest “same build” signal.

## CI sketch

```yaml
- uses: actions/checkout@v4
  with: { path: master }
- uses: actions/checkout@v4
  with:
    repository: org/split
    path: split
    token: ${{ secrets.MULTI_REPO_READ_TOKEN }}
- run: cargo install --path cargo-tog --locked
- run: cargo-tog dep-drift --master master --other split --json
```

## Limits

- Not a full Cargo resolver (features, target-cfg, renamed packages)  
- Multi-line TOML tables for a single dep are not fully supported  
- Lock comparison is **direct-deps only** by default (noise control)  
- Does not replace `cargo tree` / `cargo deny` / hakari  

For full graph policy use **cargo-deny** / **cargo-hakari**; dep-drift is for
**cross-tree pin alignment**.
