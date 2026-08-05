# Multi-repo **code** sync

## Short answers

| Question | Answer |
|----------|--------|
| Can cargo-tog sync multi-repo code? | **Yes, optionally** — partial file mirrors defined in config. |
| Does caching **need** code sync? | **No.** Object/registry cache works with zero source sharing. |
| When do you need code sync? | Only if you **copy** the same source into more than one git repo. |

## Cache vs code (do not mix them up)

```text
cargo-tog cache     →  reuse downloads + compiled objects across builds/repos
cargo-tog sync      →  keep *chosen files* identical across checkouts (mirrors)
```

- **Two independent services** that share a CLI name.
- Sharing a remote object bucket does **not** require repos to have the same tree.
- Syncing code does **not** replace a compiler cache.

## When you **do not** need sync

- One monorepo only  
- Polyrepos that are **different products** (no shared source files)  
- Splits that depend on **published crates** / git deps (Cargo is the “sync”)  
- You only want faster CI via cache  

→ Use **cache only**. Skip `[[sync.mirrors]]`.

## When you **might** want sync

You maintain a **master** tree and one or more **split** repos that must contain
the **same file bytes** for a subset of paths (CLI surface, plugins, deploy
worker, ABI file, etc.), without making them one workspace.

Typical patterns:

| Pattern | Sync? |
|---------|--------|
| Master monorepo + thin public CLI repo (partial mirror) | Optional helper |
| “Source of truth here, deploy clone there” | Optional helper |
| Libraries published to crates.io | Prefer Cargo versions, not file sync |

If the split is **generated** by your own release pipeline, keep that pipeline —
cargo-tog sync is a simple, config-driven alternative for small mirror lists.

## How sync works (optional)

`config/cargo-tog.example.toml`:

```toml
[[sync.mirrors]]
name = "cli-surface"
# Paths relative to where you run the command (or absolute)
source_root = "../my-monorepo"
target_root = "../my-cli-repo"
# Only these files — never a full monorepo dump unless you list everything
files = [
  ["cli/src/main.rs", "src/main.rs"],
  ["cli/Cargo.toml", "Cargo.toml"],
]
```

```sh
# report drift (exit 1 if different)
node scripts/cargo-tog.mjs sync --config cargo-tog.toml --check

# copy source → target (local dirs; you commit/push in the target repo)
node scripts/cargo-tog.mjs sync --config cargo-tog.toml --apply
```

**Not included (on purpose):** automatic force-push to many remotes, rewriting
history, or merging unrelated repos. Push remains your git workflow.

## Need it for *your* setup?

| Setup | Cache | Code sync |
|-------|-------|-----------|
| Several Rust projects, shared CI minutes | Yes | No |
| Master + partial mirror repos | Yes | Only for mirrored paths |
| Pure monorepo | Yes (still useful) | No |

**Default recommendation:** enable **cache**; add **sync** only when a mirror
list already exists in your head (or in a script you maintain today).
