# GitHub Actions (cargo-tog)

## Secrets (our names)

| Secret | Purpose |
|--------|---------|
| `CARGO_TOG_BUCKET` | Remote object-cache bucket (optional) |
| `CARGO_TOG_ENDPOINT` | S3-compatible API endpoint |
| `CARGO_TOG_REGION` | Region (`auto` common for R2) |
| `CARGO_TOG_ACCESS_KEY_ID` | Access key |
| `CARGO_TOG_SECRET_ACCESS_KEY` | Secret key |

Wire into the job:

```yaml
env:
  CARGO_TOG_BUCKET: ${{ secrets.CARGO_TOG_BUCKET }}
  CARGO_TOG_ENDPOINT: ${{ secrets.CARGO_TOG_ENDPOINT }}
  CARGO_TOG_REGION: ${{ secrets.CARGO_TOG_REGION }}
  CARGO_TOG_ACCESS_KEY_ID: ${{ secrets.CARGO_TOG_ACCESS_KEY_ID }}
  CARGO_TOG_SECRET_ACCESS_KEY: ${{ secrets.CARGO_TOG_SECRET_ACCESS_KEY }}
```

The Action maps these to the cache engine. You only maintain **CARGO_TOG_*** names.

## Action inputs

| Input | Default | Notes |
|-------|---------|--------|
| `compiler-cache` | `true` | Object cache + `cargo-tog-rustc` wrapper |
| `registry-cache` | `true` | crates.io / git |
| `cache-targets` | `false` | Keep false |
| `install-nextest` | `false` | Optional; same compiler cache |
| `key` / `shared-key` | | Registry cache keys |

## nextest

```yaml
- uses: your-org/cargo-tog/action@main
  with:
    key: test-${{ runner.os }}
    install-nextest: "true"
- run: cargo nextest run --workspace
```

No separate “nextest cache.” Compile reuse is the compiler cache.
