# Advanced: partial code mirrors (`sync`)

> **Not a main feature.** cargo-tog’s product is **build cache**.  
> File sync exists for rare split-repo layouts. Most teams should **ignore this
> page**. Caching does **not** depend on sync.

## When (rarely) you might use it

You intentionally keep **the same file bytes** in two git repositories (partial
mirror). You already maintain a list of paths. You want a small check/apply
helper.

If dependencies are shared via **crates.io / git deps / one monorepo**, you do
**not** need sync.

## When not to use it

- Independent products  
- Cache-only acceleration  
- Publishing libraries (use Cargo versions)  
- Anything that needs merge policy, code owners, or automated multi-remote push  

## Usage

```toml
# cargo-tog.toml — optional section
[[sync.mirrors]]
name = "example"
source_root = "../main"
target_root = "../split"
files = [
  ["path/in/main.rs", "path/in/split.rs"],
]
```

```sh
node scripts/cargo-tog.mjs sync --config cargo-tog.toml --check
node scripts/cargo-tog.mjs sync --config cargo-tog.toml --apply  # then git commit yourself
```

## Support level

Best-effort utility. No SLA. Prefer dedicated monorepo tooling
(cargo-rail, custom release bots) for serious multi-package publish graphs.
