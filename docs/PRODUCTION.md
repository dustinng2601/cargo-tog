# Production / enterprise runbook

Cross-OS architecture: [CROSS_OS.md](CROSS_OS.md).

## Readiness checklist

### Phase 0 — Quick setup, **no bucket** (day one)

- [ ] Install [Action](../action/action.yml) (or vendor under `.github/actions/cargo-tog`)
- [ ] `mode: github` (or `auto` on GitHub Actions) — **zero cloud account**
- [ ] Registry key includes OS/arch: `test-${{ runner.os }}-${{ runner.arch }}`
- [ ] Pin Rust toolchain
- [ ] `cargo test` / `cargo nextest` work unchanged
- [ ] Laptop: `mode=local` (auto when not in CI) + `RUSTC_WRAPPER=cargo-tog-rustc`
- [ ] Optional minimal: `mode: registry-only` if you refuse an object-cache engine

See [MODES.md](MODES.md).

### Phase 1 — Optional: multi-repo remote objects (bucket)

Only when GitHub-hosted cache is not enough (evictions, multi-repo, durable store).

- [ ] S3-compatible bucket (R2/S3/MinIO) near CI runners
- [ ] Org secrets `CARGO_TOG_BUCKET` / `ENDPOINT` / `REGION` / keys
- [ ] `mode: remote` or auto with bucket set
- [ ] Matrix keys: `test-${{ runner.os }}-${{ runner.arch }}-${{ matrix.target }}`
- [ ] Warm builds **per OS**; no object sharing across triples (by design)

### Phase 2 — Hardening

- [ ] Separate **CI** credentials from **developer** credentials (optional two buckets)
- [ ] Lifecycle policy: expire unused objects (e.g. 30–90 days)
- [ ] Alert on bucket size / cost
- [ ] Document on-call: “cache miss storm” = rustc upgrade or `RUSTFLAGS` change
- [ ] Self-hosted runners: persistent `CARGO_HOME` + local object dir + remote backfill

### Phase 3 — Optional (not cache)

- [ ] Dependency drift jobs between related trees (`dep-drift`)
- [ ] Partial file mirrors only if you already run split-repo copies (`sync` — advanced)

## SLO-oriented expectations

| Metric | Healthy signal |
|--------|----------------|
| Cold CI after toolchain bump | Full recompile expected once |
| Warm PR on same rustc/target | Material reduction in rustc time |
| Registry restore | Seconds, not minutes |
| Cache write failures | Job still green if engine degrades? Prefer fail-open for object put optional; fail-closed for auth misconfig is OK |

Object cache is **best-effort acceleration**. Correctness always comes from Cargo’s
normal build graph—not from trusting remote objects blindly (engines verify by
content key).

## Security

| Topic | Guidance |
|-------|----------|
| Secrets | Org or repo Actions secrets only; never commit |
| Bucket ACL | Private; no public read |
| Poisoning | Prefer CI-only write keys for production cache |
| Supply chain | Pin Action versions (`@vX` / commit SHA) in enterprise orgs |
| PII | Do not store source trees in the object bucket—objects are compile units |

## Failure modes

| Symptom | Likely cause | Action |
|---------|--------------|--------|
| 0% hits after change | rustc / flags / target changed | Expected; warm again |
| Auth errors on remote | Wrong key/endpoint | Fix secrets; temporary empty bucket falls back to GHA mode if configured that way |
| GH cache full | Still caching `target/` | Ensure `cache-targets: false` |
| Slow “hits” | Cross-region bucket | Move bucket or runners |
| nextest “not cached” | Confused with test result cache | nextest uses compile cache only |

## Capacity planning

| Workload | Guidance |
|----------|----------|
| Small team, 1–3 repos | Phase 0 may be enough |
| Many repos, shared deps | Phase 1 remote bucket |
| Huge monorepo, multi-lang | Evaluate Bazel remote cache separately |
| Docker-heavy services | cargo-chef **plus** cargo-tog for non-Docker CI |

## Compliance notes

- Cache contents are **build artifacts**, not source of truth.  
- Rebuild from lockfile + toolchain must always be possible with cache disabled.  
- Document cache disable switch for forensic clean builds: `compiler-cache: false`.
