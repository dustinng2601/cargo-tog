# GitHub Actions recipes

## Pattern used by sd-soundseek (good baseline)

```yaml
env:
  CARGO_TERM_COLOR: always
  CARGO_PROFILE_DEV_DEBUG: "0"   # shrink artifacts; no debugger in CI
  RUSTC_WRAPPER: sccache
  SCCACHE_GHA_ENABLED: "true"

steps:
  - uses: mozilla-actions/sccache-action@v0.0.9   # or current
  - uses: Swatinem/rust-cache@v2
    with:
      workspaces: ". -> target"
      cache-targets: false   # critical: don't upload target/
      cache-bin: false
```

## Using cargo-tog composite action

```yaml
- uses: sd2ek/cargo-tog/action@main
  with:
    sccache: "true"
    rust-cache: "true"
    # When org R2 is ready, set secrets and:
    # sccache-remote: "true"
```

## True multi-repo compile share (R2 / S3)

GHA’s sccache backend is convenient but **not** a durable org-wide store.

1. Create bucket `soundseek-sccache` (or `sd2ek-sccache`).  
2. Org secrets: `SCCACHE_BUCKET`, keys, endpoint for R2.  
3. In every Rust workflow:

```yaml
env:
  RUSTC_WRAPPER: sccache
  SCCACHE_BUCKET: ${{ secrets.SCCACHE_BUCKET }}
  SCCACHE_ENDPOINT: ${{ secrets.SCCACHE_ENDPOINT }}
  SCCACHE_REGION: auto
  AWS_ACCESS_KEY_ID: ${{ secrets.SCCACHE_AWS_ACCESS_KEY_ID }}
  AWS_SECRET_ACCESS_KEY: ${{ secrets.SCCACHE_AWS_SECRET_ACCESS_KEY }}
  # optional: SCCACHE_S3_USE_SSL: true
```

4. **Do not** enable GHA backend at the same time as S3 for the same job
   (one backend wins; keep it simple).

## Per-repo vs org cache limits

| Store | Scope | Limit pain |
|-------|--------|------------|
| GH Actions cache | Per repo | 10 GB class limits; evictions |
| sccache GHA | Tied to GHA cache behavior | OK for one big monorepo |
| sccache S3/R2 | Org | You pay storage; you control eviction |

## Matrix builds

Same sccache remote helps **linux / macos / windows** only within each triple
(objects are target-specific). That’s fine — still shares deps **across repos**
on the same OS/target.

## Polyrepo that is mostly JS (workers)

Skip rust-cache/sccache. cargo-tog is for Cargo-heavy repos only.

## Drift job (master)

Weekly on master:

```yaml
- run: node path/to/cargo-tog/scripts/cargo-tog.mjs dep-drift --master . --other ../soundseek-cli
```

Or checkout multiple repos with a PAT and compare.
