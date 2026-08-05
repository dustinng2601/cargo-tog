# GitHub Actions

## Baseline (no remote bucket)

```yaml
env:
  CARGO_TERM_COLOR: always
  CARGO_PROFILE_DEV_DEBUG: "0"
  CARGO_INCREMENTAL: "0"

steps:
  - uses: your-org/cargo-tog/action@main
    with:
      key: test-${{ runner.os }}-${{ runner.arch }}
  - run: cargo test --workspace
  # or: cargo nextest run --workspace
```

The Action sets `RUSTC_WRAPPER=sccache` and enables the **GHA sccache backend**
when `SCCACHE_BUCKET` is unset.

## Remote sccache (multi-repo reuse)

Org or repo secrets:

| Secret | Maps to env | Notes |
|--------|-------------|--------|
| `SCCACHE_BUCKET` | `SCCACHE_BUCKET` | Bucket name |
| `SCCACHE_ENDPOINT` | `SCCACHE_ENDPOINT` | R2/S3 endpoint URL |
| `SCCACHE_REGION` | `SCCACHE_REGION` | Often `auto` for R2 |
| `SCCACHE_AWS_ACCESS_KEY_ID` | `AWS_ACCESS_KEY_ID` | S3-compatible key |
| `SCCACHE_AWS_SECRET_ACCESS_KEY` | `AWS_SECRET_ACCESS_KEY` | Secret |

```yaml
env:
  SCCACHE_BUCKET: ${{ secrets.SCCACHE_BUCKET }}
  SCCACHE_ENDPOINT: ${{ secrets.SCCACHE_ENDPOINT }}
  SCCACHE_REGION: ${{ secrets.SCCACHE_REGION }}
  AWS_ACCESS_KEY_ID: ${{ secrets.SCCACHE_AWS_ACCESS_KEY_ID }}
  AWS_SECRET_ACCESS_KEY: ${{ secrets.SCCACHE_AWS_SECRET_ACCESS_KEY }}
```

Empty secrets → automatic GHA backend (safe default while you provision storage).

## nextest

```yaml
- uses: your-org/cargo-tog/action@main
  with:
    key: test-${{ runner.os }}
    install-nextest: "true"
- run: cargo nextest run --workspace --all-features
```

Or install nextest yourself (`taiki-e/install-action`); sccache still applies.

## Vendoring the Action

If you cannot `uses:` a private Action repo, copy `action/` to
`.github/actions/cargo-cache/` in each project and `uses: ./.github/actions/cargo-cache`.
