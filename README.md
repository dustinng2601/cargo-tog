# cargo-tog

**Cargo, together.** Multi-repo Rust **build cache** coordination, plus optional
**partial code mirrors** — under **your** names, for any org or laptop.

```text
cargo-tog cache   →  reuse crate downloads + compiler objects (CI + local)
cargo-tog sync    →  optional: keep listed files identical across repos
```

Caching does **not** require code sync. Code sync is only for partial mirrors.

## Public names (ours)

| You say / set | Meaning |
|---------------|---------|
| **cargo-tog** | This project + CLI + CI Action |
| **compiler cache** | Object-level compile reuse across jobs/repos |
| **registry cache** | crates.io / git download reuse |
| **`CARGO_TOG_BUCKET`** etc. | Your secrets for remote object storage |
| **`cargo-tog-rustc`** | Wrapper set as `RUSTC_WRAPPER` in CI |

You do **not** configure day-to-day life in terms of other tools’ product names.
Engines under the hood are an implementation detail of the Action.

## Share matrix (cache)

| Layer | Share across repos? |
|-------|---------------------|
| Registry / git downloads | **Yes** |
| Compiler objects (remote bucket) | **Yes** |
| Full `target/` directory | **No** (per workspace only) |

## Multi-repo code sync — need it?

| | |
|--|--|
| **Need it for cache?** | **No** |
| **Need it if each repo is independent?** | **No** |
| **Need it if you copy the same sources into split git repos?** | **Optional helper** — see [docs/SYNC.md](docs/SYNC.md) |

If you only want faster builds: **cache only**.

## Quick start

```sh
node scripts/cargo-tog.mjs doctor
node scripts/cargo-tog.mjs cache-plan
node scripts/cargo-tog.mjs inventory --root /path/to/workspace
node scripts/cargo-tog.mjs sync --config cargo-tog.toml --check   # optional mirrors
```

### CI

```yaml
env:
  CARGO_TOG_BUCKET: ${{ secrets.CARGO_TOG_BUCKET }}
  CARGO_TOG_ENDPOINT: ${{ secrets.CARGO_TOG_ENDPOINT }}
  CARGO_TOG_REGION: ${{ secrets.CARGO_TOG_REGION }}
  CARGO_TOG_ACCESS_KEY_ID: ${{ secrets.CARGO_TOG_ACCESS_KEY_ID }}
  CARGO_TOG_SECRET_ACCESS_KEY: ${{ secrets.CARGO_TOG_SECRET_ACCESS_KEY }}

steps:
  - uses: actions/checkout@v4
  - uses: dtolnay/rust-toolchain@stable
  - uses: your-org/cargo-tog/action@main
    with:
      key: test-${{ runner.os }}-${{ runner.arch }}
      install-nextest: "true"   # optional; same compiler cache
  - run: cargo nextest run --workspace
```

Empty bucket secrets → GitHub-hosted object cache (still fine per repo).  
Set the bucket once at **org** level → multi-repo compile reuse.

## Docs

- [docs/LAYERS.md](docs/LAYERS.md) — what to share  
- [docs/SYNC.md](docs/SYNC.md) — code mirrors (optional)  
- [docs/GITHUB_ACTIONS.md](docs/GITHUB_ACTIONS.md)  
- [docs/LOCAL_DEV.md](docs/LOCAL_DEV.md)  
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)  

## License

MIT (`LICENSE-MIT`).
