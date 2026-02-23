# Security Policy

## Supported Versions

Security fixes are provided for the latest release series.

## Reporting a Vulnerability

Please do not open public issues for sensitive vulnerabilities.

Use private disclosure channels first and include:

- affected version
- platform details
- reproduction steps
- impact assessment

## Security Boundaries

- The app reads local session files under `~/.codex/sessions`.
- It communicates with Discord over local IPC.
- It does not intentionally transmit data to third-party telemetry services.

## Hardening Notes

- Single-instance lock to reduce duplicate writers.
- Defensive JSON parsing for schema drift.
- No default logging of secrets or auth tokens.
