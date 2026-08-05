# GitHub Actions integration

## Secrets

| Secret | Required | Purpose |
|--------|----------|---------|
| `CARGO_TOG_BUCKET` | No | Remote multi-repo objects |
| `CARGO_TOG_ENDPOINT` | With bucket | S3-compatible endpoint |
| `CARGO_TOG_REGION` | No | Default `auto` when bucket set |
| `CARGO_TOG_ACCESS_KEY_ID` | With bucket | Access key |
| `CARGO_TOG_SECRET_ACCESS_KEY` | With bucket | Secret |

Wire on every Rust job (org secrets preferred):

```yaml
env:
  CARGO_TOG_BUCKET: ${{ secrets.CARGO_TOG_BUCKET }}
  CARGO_TOG_ENDPOINT: ${{ secrets.CARGO_TOG_ENDPOINT }}
  CARGO_TOG_REGION: ${{ secrets.CARGO_TOG_REGION }}
  CARGO_TOG_ACCESS_KEY_ID: ${{ secrets.CARGO_TOG_ACCESS_KEY_ID }}
  CARGO_TOG_SECRET_ACCESS_KEY: ${{ secrets.CARGO_TOG_SECRET_ACCESS_KEY }}
```

## Action inputs

| Input | Default | Description |
|-------|---------|-------------|
| `compiler-cache` | `true` | Object cache + `cargo-tog-rustc` |
| `registry-cache` | `true` | crates.io / git |
| `cache-targets` | `false` | Keep false in production |
| `install-nextest` | `false` | Install nextest binary |
| `key` / `shared-key` | | Registry cache keys |
| `incremental` | `0` | `CARGO_INCREMENTAL` |
| `fail-on-cache-error` | `false` | Reserved for strict modes |

## Enterprise pinning

```yaml
- uses: your-org/cargo-tog/action@v0.1.0
# or
- uses: your-org/cargo-tog/action@<full-commit-sha>
```

## nextest

```yaml
- uses: your-org/cargo-tog/action@main
  with:
    key: test-${{ runner.os }}-${{ runner.arch }}
    install-nextest: "true"
- run: cargo nextest run --workspace --all-features
```
