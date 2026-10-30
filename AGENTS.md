# AI Project Context

## Purpose
`modern-signal-cli-api` (Modern Signal CLI API, a continuation of h4x0r/signal-cli-api) is a native Rust/axum REST, streaming, webhook, and metrics wrapper around signal-cli. It is used by the Signal automation stack but is also a standalone general-purpose project.

## Architecture
- Rust + axum HTTP API.
- Maintains a persistent connection to signal-cli rather than restarting the JVM for every request.
- Supports WebSocket, SSE, webhooks, Prometheus metrics, TLS, groups/contacts/accounts/devices, and message operations.
- Integration tests use a mock signal-cli daemon and should not require a real Signal account.

## Local deployment context
- Six packages this API together with signal-cli for `mikesimone/signal-forwarder`.
- Multiple Signal accounts share the production daemon, so account-selection semantics are important to downstream callers.

## Working rules
- Read `README.md` before API or CLI changes.
- Preserve backward compatibility for existing endpoints unless a breaking change is explicitly requested.
- Keep request tracing/observability intact.
- Do not add real Signal credentials or account state to tests or Git.
- Run `cargo test` for behavioral changes and update docs/OpenAPI-facing behavior when endpoints or flags change.

## Dependency policy (2026-10-30)
- signal-cli is a **pinned, stock dependency**: never fork or patch it. It is downloaded by the `Dockerfile` from the official release, verified by SHA-256. Upgrading it = bump `SIGNAL_CLI_VERSION` and `SIGNAL_CLI_SHA256` together, run the tests, check route params against that release.
- The test mock must be as strict as real signal-cli: mocks that accept any params hid the `finishLink` and `getAttachment` bugs.
