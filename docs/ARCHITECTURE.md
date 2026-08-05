# Architecture

## Two independent features

| Feature | Purpose | Required? |
|---------|---------|-----------|
| **Cache** | Reuse downloads + compiler objects | Core product |
| **Sync** | Keep listed files identical across trees | Optional |

Caching **never** depends on sync. Sync **never** replaces a compiler cache.

## Cache

1. Registry/git under `CARGO_HOME`  
2. Compiler objects via remote bucket (`CARGO_TOG_*`) or GitHub-hosted fallback  
3. Public wrapper name: **`cargo-tog-rustc`**  
4. No shared `target/` across workspaces  

## Sync (optional)

Config-driven partial file mirrors (`[[sync.mirrors]]`). Local copy only;
you commit/push. Use only when the same source must live in two git repos.

## Non-goals

- One mega-workspace for every repo  
- Auto force-push to many remotes  
- Product-specific branding  
