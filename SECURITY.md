# Security

## Reporting

Report vulnerabilities privately to the repository maintainers. Do not open
public issues for credential or cache-poisoning scenarios.

## Threat model (cache)

| Threat | Mitigation |
|--------|------------|
| Public bucket read | Private ACL; no public policies |
| Leaked CI write keys | Org secret rotation; least-privilege IAM |
| Untrusted writers filling CI cache | Separate credentials / buckets for CI vs laptops |
| Believing cache is SBoM | Cache is acceleration only; rebuild from lockfile for release attestation |

## Secrets

Never commit `CARGO_TOG_SECRET_ACCESS_KEY` or any cloud credential. Use platform
secret stores (GitHub Actions org secrets, Vault, etc.).
