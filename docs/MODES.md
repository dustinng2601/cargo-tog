# Cache modes (bucket optional)

Not everyone wants R2/S3. cargo-tog supports **four acceleration levels** plus
**off**. Pick the lightest that fits.

## Comparison

| Mode | Outside cloud deps? | Multi-repo object reuse? | Best for |
|------|---------------------|---------------------------|----------|
| **`github`** | **None** (GitHub only) | Same repo / limited GHA scope | **Quick CI setup** |
| **`local`** | **None** (disk) | Same machine / persistent runner | Laptops, self-hosted |
| **`registry-only`** | None | No (downloads only) | Minimal: skip object engine |
| **`remote`** | Bucket + keys | **Yes** (same triple) | Many repos, shared CI farm |
| **`off`** | None | No | Clean / forensic builds |

## How to choose

```text
Just want faster CI today, no AWS/R2?
  → mode=github   (default on GitHub Actions when no bucket)

Laptop / one self-hosted runner with a disk?
  → mode=local    (default offline)

Want zero compiler-cache binary / engine?
  → mode=registry-only

Many repos, shared compiles across jobs?
  → mode=remote   (set CARGO_TOG_BUCKET)

Debugging a clean build?
  → mode=off
```

## Configuration

### Environment

```bash
# explicit
export CARGO_TOG_MODE=github    # or local | remote | registry-only | off

# auto (default):
#   bucket set     → remote
#   GITHUB_ACTIONS → github
#   else           → local
```

### GitHub Action

```yaml
# Quick path — no secrets, no bucket
- uses: your-org/cargo-tog/action@main
  with:
    mode: github          # or omit: auto does this on GHA
    key: test-${{ runner.os }}-${{ runner.arch }}

# Downloads only (no object engine install)
- uses: your-org/cargo-tog/action@main
  with:
    mode: registry-only
    key: test-${{ runner.os }}-${{ runner.arch }}

# Multi-repo later (when you have a bucket)
- uses: your-org/cargo-tog/action@main
  with:
    mode: remote   # or auto + CARGO_TOG_BUCKET secret
```

## What each mode does

### `github` — zero bucket (recommended first CI step)

1. Install compiler-cache engine  
2. Point it at **GitHub Actions cache** as object backend  
3. Registry cache via rust-cache (`cache-targets: false`)  
4. `RUSTC_WRAPPER=cargo-tog-rustc`  

**Outside deps:** GitHub only.  
**Limits:** GHA cache quotas/evictions; not a durable multi-org object store.  
**Still huge win** vs cold compiles every job.

### `local` — zero cloud

1. Objects under `CARGO_TOG_CACHE_DIR` (platform default)  
2. Registry: existing `CARGO_HOME`  
3. No bucket, no GHA object backend  

**Outside deps:** disk.  
**Ideal for:** developers, self-hosted runners with persistent volumes.

### `registry-only` — minimal

1. **No** compiler object cache, **no** engine binary  
2. Only restore crates.io / git downloads  
3. Plain `rustc` (no wrapper)  

**Outside deps:** none.  
**Ideal for:** simple repos, or when you refuse extra binaries.

### `remote` — multi-repo

Requires `CARGO_TOG_BUCKET` (+ endpoint/keys). See PRODUCTION.md Phase 1.

### `off`

Disable cargo-tog acceleration entirely (`compiler-cache`/`registry-cache` off).

## Migration path (typical team)

```text
Day 1     mode=github          (or local on laptop)
Week 2    keep github; tune keys for OS matrix
Later     mode=remote          when CI $ or multi-repo pain justifies a bucket
Never     mode=registry-only   if object cache isn’t worth it
```

You can stay on **`github` or `local` forever**. Bucket is an **upgrade**, not a requirement.

## CLI

```sh
cargo-tog doctor        # prints resolved mode
cargo-tog cache-plan    # includes mode table
echo $CARGO_TOG_MODE
```
