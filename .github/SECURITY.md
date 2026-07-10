# Security Policy

## Supported versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

## Reporting a vulnerability

PDF parsing is a high-risk domain (malformed streams, expansion bombs, etc.).

Please **do not** open a public issue for exploitable vulnerabilities.

Email: gauticse2015@gmail.com with:

- Description and impact
- Reproduction steps / minimal PDF if possible
- Affected commit or version

We will acknowledge receipt and work on a fix or mitigation timeline.

## Hardening notes

- Prefer process isolation when parsing untrusted uploads.
- v0.1 rejects encrypted PDFs; do not assume decryption or JS execution.
