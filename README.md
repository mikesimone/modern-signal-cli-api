# Modern Signal CLI API

REST, WebSocket, SSE and webhook bridge for [signal-cli](https://github.com/AsamK/signal-cli), packaged as
**one Docker image** that pins a stock signal-cli release (see `Dockerfile`). Continuation of
h4x0r/signal-cli-api (see `NOTICE.md`). signal-cli itself is never forked or patched; updating it is a
version + checksum bump in the `Dockerfile`.

Build: `docker build -t modern-signal-cli-api .` (reproducible from this repo alone: Rust build stage +
checksummed signal-cli download). Run: mount a data dir at `/var/lib/signal-cli-api` and publish 8080.
The binary is `modern-signal-cli-api`; it replaces `signal-cli-api` (same flags, same routes).
**It has no authentication: keep port 8080 off untrusted networks.**

---

## Staying current

signal-cli is pinned in the `Dockerfile` (version + SHA-256). `.github/workflows/signal-cli-bump.yml` checks for a
new signal-cli release every Monday, runs the full tests and the contract check against it, and only then puts the
bump on `main`; if anything fails it opens an issue and changes nothing. `contract/bump-signal-cli.sh [version]`
does the same edit by hand.

## Contract tests

The test-suite talks to a *fake* signal-cli, which accepts anything. `contract/run.sh` checks our routes against the
**real** one instead: it reads every JSON-RPC method and parameter name from the pinned signal-cli in the image
(`contract/SpecDump.java`), records every call our tests make (`CONTRACT_CAPTURE`), and fails if a route calls a
method signal-cli does not have or sends a parameter name it does not read (`contract/check.py`). That second case
is the dangerous one: signal-cli silently ignores unknown parameters, so a typo does nothing and no error appears.
Known exceptions are listed, with reasons, in `contract/known-gaps.txt` (the list can only shrink: stale entries
fail too). **After bumping `SIGNAL_CLI_VERSION` in the Dockerfile, run `contract/run.sh` before merging.**

---

# signal-cli-api

A native REST + WebSocket API for [signal-cli](https://github.com/AsamK/signal-cli). Build Signal bots, notification systems, and integrations in any language.

## Why this one

- **Zero config.** Run `signal-cli-api` and it works. Auto-spawns signal-cli, picks free ports, no containers or orchestration needed.
- **Real-time streaming.** WebSocket, SSE, and webhook delivery for incoming messages. No polling loops.
- **Observable.** Prometheus metrics, structured request tracing with `x-request-id`, per-RPC latency logging.
- **Native TLS.** Pass `--tls-cert` and `--tls-key`. No reverse proxy needed for HTTPS.
- **Fast.** Rust + axum. Sub-millisecond request overhead. Persistent JSON-RPC connection to signal-cli (no JVM restarts per request).
- **Tested.** 267 integration tests against a mock signal-cli daemon.

## Quick start

### 1. Install

```bash
cargo install signal-cli-api
```

### 2. Set up signal-cli and link your phone

```bash
brew install signal-cli          # or your package manager
brew install qrencode            # to display QR code in terminal
```

Link signal-cli to your existing Signal account (keeps your phone working):

```bash
signal-cli link -n "my-server" | tee >(xargs -L 1 qrencode -t utf8)
```

This prints a QR code in your terminal. Scan it with your phone:
**Signal app > Settings > Linked Devices > Link New Device**.

Wait for the process to exit — linking is complete. Then sync your contacts:

```bash
signal-cli -u +1234567890 receive
```

### 3. Run

```bash
signal-cli-api
```

That's it. API is live at `http://localhost:8080`. signal-cli is managed automatically.

If you already run signal-cli as a daemon:

```bash
signal-cli-api --signal-cli 127.0.0.1:7583
```

### Options

```
--signal-cli <addr>   Connect to existing signal-cli daemon (default: auto-spawn)
--listen <addr>       HTTP listen address (default: 127.0.0.1:8080, falls back to random port if busy)
--tls-cert <path>     TLS certificate (PEM). Enables HTTPS.
--tls-key <path>      TLS private key (PEM). Required with --tls-cert.
```

## Send a message

```bash
curl -X POST http://localhost:8080/v2/send \
  -H 'Content-Type: application/json' \
  -d '{
    "message": "Hello from my app!",
    "number": "+1234567890",
    "recipients": ["+1987654321"]
  }'
```

```json
{"timestamp": 1234567890}
```

### The `send` contract: raw passthrough by design

`/v1/send` and `/v2/send` are a **deliberate, undocumented-field-friendly
passthrough** to signal-cli's `send` JSON-RPC method — the request body is
forwarded to signal-cli almost verbatim (see "Attachments" below for the one
exception), with no field whitelist. This is a decided design choice, not an
oversight: it's what lets every signal-cli `send` capability work here
immediately, including ones this README doesn't spell out, without waiting
on a matching typed field to be added to this project first.

Field names match signal-cli's own JSON-RPC parameter names (the same names
you'd use with `signal-cli --output=json jsonRpc`), **not** a REST-ified
alias set — with one built-in convenience: `recipients` (plural) is accepted
as an alias for `recipient`, courtesy of signal-cli's own JSON-RPC parameter
binding (it falls back from a missing `recipient` key to `recipients`). No
other pluralized/renamed alias is supported. In particular:

- `account` is required (not `number`) — signal-cli only resolves the
  sending account from a top-level `account` field. On a daemon with more
  than one registered account (this deployment has two), a `number` field
  is silently ignored and the call fails with "Method requires valid
  account parameter."
- `attachment` is the real field for attachments (not `base64_attachments`),
  and takes an array of either data URIs
  (`data:<mime>;filename=<name>;base64,<data>`) or file paths already on the
  signal-cli host.

Commonly-used fields beyond the basics, confirmed to work purely because the
passthrough has no whitelist:

| Field | Type | Purpose |
|-------|------|---------|
| `editTimestamp` | number | Edit a previously-sent message in place |
| `quoteTimestamp` / `quoteAuthor` / `quoteMessage` | number / string / string | Quote a prior message |
| `storyTimestamp` / `storyAuthor` | number / string | Reply to a story |
| `mention` | array of `"start:length:recipient"` strings | @-mention a group member |
| `textStyle` | array of `"start:length:STYLE"` strings | Bold/italic/strikethrough/spoiler/monospace spans |
| `noUrgent` | bool | Suppress the push notification (still delivered in real time if the app is open) |
| `viewOnce` | bool | Send as a view-once message |
| `sticker` | `"packId:stickerId"` string | Send a sticker |
| `previewUrl` / `previewTitle` / `previewDescription` / `previewImage` | strings | Attach a link preview |
| `voiceNote` | bool | Mark an audio attachment as a voice note |
| `groupId` | string | Send to a group instead of `recipient` |
| `noteToSelf` | bool | Send to your own account |

This isn't an exhaustive list — any parameter signal-cli's `send` JSON-RPC
method accepts works, whether or not it's named above. See
[`SendCommand.java`](https://github.com/AsamK/signal-cli/blob/master/src/main/java/org/asamk/signal/commands/SendCommand.java)
in signal-cli for the canonical, versioned source of truth. If a future
refactor ever replaces this passthrough with a typed request struct, every
field in this table (plus `account`/`recipient`/`message`/`attachment`) must
keep working, or it's a breaking change for production callers depending on
it today.

## Receive messages

### WebSocket (recommended for bots)

Connect to `ws://localhost:8080/v1/receive/+1234567890`. Messages stream as JSON:

```json
{
  "envelope": {
    "source": "+1987654321",
    "dataMessage": {
      "message": "Hey!",
      "timestamp": 1234567890
    }
  }
}
```

Works with any WebSocket client — Python, Node, Go, Rust, whatever.

### Server-Sent Events (SSE)

```bash
curl -N http://localhost:8080/v1/events/+1234567890
```

### Webhooks

Push incoming messages to your HTTP endpoint:

```bash
# Register
curl -X POST http://localhost:8080/v1/webhooks \
  -H 'Content-Type: application/json' \
  -d '{"url": "https://your-app.com/signal-hook"}'

# With event filtering
curl -X POST http://localhost:8080/v1/webhooks \
  -H 'Content-Type: application/json' \
  -d '{"url": "https://your-app.com/hook", "events": ["message", "receipt"]}'
```

## Monitoring

Prometheus-compatible metrics at `/metrics`:

```
signal_messages_sent_total 42
signal_messages_received_total 108
signal_rpc_calls_total 312
signal_rpc_errors_total 0
signal_ws_clients_active 2
```

Every request gets an `x-request-id` header and structured log entry:

```
INFO request_id=47 method=POST path="/v2/send" status=201 latency_ms=1152
INFO rpc_method="send" status=201 latency_ms=1150
```

## API reference

### Messages

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/v2/send` | Send message (text, attachments, mentions, quotes) |
| POST | `/v1/send` | Send message (v1, deprecated) |
| GET | `/v1/receive/{number}` | WebSocket stream |
| DELETE | `/v1/remote-delete/{number}` | Delete a sent message (body: `recipient`/`groupId`, `targetTimestamp`; legacy `timestamp` is translated) |

### Typing, Reactions & Receipts

| Method | Endpoint | Description |
|--------|----------|-------------|
| PUT | `/v1/typing-indicator/{number}` | Show typing |
| DELETE | `/v1/typing-indicator/{number}` | Stop typing |
| POST | `/v1/reactions/{number}` | Send reaction (body: `recipient`/`groupId`, `emoji`, `targetAuthor`, `targetTimestamp`; legacy `reaction`, `target_author`, `timestamp` are translated) |
| DELETE | `/v1/reactions/{number}` | Remove reaction |
| POST | `/v1/receipts/{number}` | Send read/delivery receipt (body: `recipient`, `type`, `targetTimestamp`; legacy `receipt_type`, `timestamp` are translated) |

### Groups

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/v1/groups/{number}` | List groups |
| POST | `/v1/groups/{number}` | Create group |
| GET | `/v1/groups/{number}/{groupid}` | Get group |
| PUT | `/v1/groups/{number}/{groupid}` | Update group |
| DELETE | `/v1/groups/{number}/{groupid}` | Delete group |
| POST | `/v1/groups/{number}/{groupid}/members` | Add members |
| DELETE | `/v1/groups/{number}/{groupid}/members` | Remove members |
| POST | `/v1/groups/{number}/{groupid}/admins` | Add admins |
| DELETE | `/v1/groups/{number}/{groupid}/admins` | Remove admins |
| POST | `/v1/groups/{number}/join` (also `/{groupid}/join`) | Join a group by invite link: body `{"uri": "https://signal.group/#..."}` (signal-cli cannot join by group id; without `uri` you get a 400 saying so) |
| POST | `/v1/groups/{number}/{groupid}/quit` | Quit group |
| POST | `/v1/groups/{number}/{groupid}/block` | Block group |
| GET | `/v1/groups/{number}/{groupid}/avatar` | Get group avatar |
| DELETE | `/v1/groups/{number}/{groupid}/messages` | Admin-delete a message (requires group admin) |

### Contacts

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/v1/contacts/{number}` | List contacts |
| GET | `/v1/contacts/{number}/{recipient}` | Get contact |
| PUT | `/v1/contacts/{number}` | Update contact |
| DELETE | `/v1/contacts/{number}/{recipient}` | Remove contact |
| POST | `/v1/contacts/{number}/sync` | Sync contacts |
| GET | `/v1/contacts/{number}/{recipient}/avatar` | Get contact avatar |
| POST | `/v1/contacts/{number}/{recipient}/block` | Block contact |
| DELETE | `/v1/contacts/{number}/{recipient}/block` | Unblock contact |
| PUT | `/v1/contacts/{number}/message-requests` | Accept/delete a message request |

### Accounts

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/v1/accounts` | List accounts |
| POST | `/v1/register/{number}` | Register |
| POST | `/v1/register/{number}/verify/{token}` | Verify |
| POST | `/v1/unregister/{number}` | Unregister |
| POST | `/v1/accounts/{number}/rate-limit-challenge` | Rate-limit challenge |
| PUT | `/v1/accounts/{number}/settings` | Update account attributes (device name, unidentified/discoverability/number-sharing) |
| POST | `/v1/accounts/{number}/pin` | Set PIN |
| DELETE | `/v1/accounts/{number}/pin` | Remove PIN |
| POST | `/v1/accounts/{number}/username` | Set username |
| DELETE | `/v1/accounts/{number}/username` | Remove username |
| POST | `/v1/accounts/{number}/sync-request` | Request a full sync from the primary device |
| POST | `/v1/accounts/{number}/number` | Start a phone number change |
| POST | `/v1/accounts/{number}/number/verify` | Finish a phone number change |

### Devices

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/v1/devices/{number}` | List devices |
| POST | `/v1/devices/{number}` | Link device |
| PUT | `/v1/devices/{number}/{device_id}` | Rename a linked device |
| DELETE | `/v1/devices/{number}/{device_id}` | Remove device |
| DELETE | `/v1/devices/{number}/local-data` | Delete local data |
| POST | `/v1/devices/{number}/add` | Approve linking a new device (primary device only) |
| GET | `/v1/qrcodelink` | QR code link URI (`?device_name=` is remembered and used when you finish the link with `POST /v1/devices/{number}`, since signal-cli only takes the name then) |
| GET | `/v1/qrcodelink/raw` | Raw link URI |

### Identities & Profiles

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/v1/identities/{number}` | List identities |
| PUT | `/v1/identities/{number}/trust/{number_to_trust}` | Trust identity |
| PUT | `/v1/profiles/{number}` | Update profile |

### Polls, Stickers & Stories

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/v1/polls/{number}` | Create poll |
| POST | `/v1/polls/{number}/vote` | Vote |
| DELETE | `/v1/polls/{number}` | Terminate (close) poll |
| GET | `/v1/sticker-packs/{number}` | List sticker packs |
| POST | `/v1/sticker-packs/{number}` | Install sticker pack (by `uri`, or `pack_id`+`pack_key`) |
| GET | `/v1/sticker-packs/{number}/{pack_id}/{sticker_id}` | Get a single sticker |
| POST | `/v1/stories/{number}` | Post a story |
| POST | `/v1/pins/{number}` | Pin a message |
| POST | `/v1/pins/{number}/unpin` | Unpin a message |
| POST | `/v1/payments/{number}` | Send a payment notification |

### Calls

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/v1/calls/{number}` | List active calls |
| POST | `/v1/calls/{number}` | Start an outgoing call |
| POST | `/v1/calls/{number}/{call_id}/accept` | Accept an incoming call |
| POST | `/v1/calls/{number}/{call_id}/reject` | Reject an incoming call |
| POST | `/v1/calls/{number}/{call_id}/hangup` | Hang up an active call |

### Attachments & Search

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/v1/attachments` | List stored attachments `[{id, size, modified}]` (built in: signal-cli has no such method yet) |
| GET | `/v1/attachments/{id}` | Get attachment (`?account=+1555...` selects the account when several are registered) |
| DELETE | `/v1/attachments/{id}` | Delete a stored attachment, 204 or 404 (built in: signal-cli has no such method yet) |
| GET | `/v1/search/{number}?numbers=+111,+222` | Check registration status |

### Webhooks

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/v1/webhooks` | Register webhook |
| GET | `/v1/webhooks` | List webhooks |
| DELETE | `/v1/webhooks/{id}` | Remove webhook |

### System

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/v1/health` | Health check (204) |
| GET | `/v1/about` | This wrapper's own version and build info |
| GET | `/v1/version` | signal-cli's own reported version |
| GET | `/v1/openapi.json` | OpenAPI 3.0 spec |
| GET | `/v1/events/{number}` | SSE stream |
| GET | `/metrics` | Prometheus metrics |

### Configuration and settings

signal-cli has no getter or setter for configuration over JSON-RPC, so these are built from what it does have:

- `POST /v1/configuration/{number}/settings` applies settings through signal-cli's real `updateConfiguration`
  (`readReceipts`, `typingIndicators`, `linkPreviews`, `unidentifiedDeliveryIndicators`) and `updateAccount`
  (`deviceName`, `numberSharing`, `discoverableByNumber`, `unrestrictedUnidentifiedSender`, `username`,
  `deleteUsername`). `snake_case` names work too. Anything else is a 400 listing what is supported.
- `GET /v1/configuration/{number}/settings` returns what was last set *through this API* (signal-cli cannot report
  its settings back), kept in `modern-signal-cli-api-settings.json` in signal-cli's data directory.
- `GET /v1/configuration` returns `{trustMode, signalCli: {version}}`. The trust mode is a daemon start-up flag, so
  `POST` only accepts the value already in force and answers 501 otherwise.

## Routes signal-cli lacks (and how they switch over later)

A few routes have no signal-cli method behind them yet (attachment list/delete, the configuration getters and
setters). Each one asks signal-cli first; if it answers "Method not implemented" the route runs the built-in version
in `src/native.rs`. When a newer signal-cli adds the method, the route starts using it with no code change, and
`contract/run.sh` fails with a stale `contract/known-gaps.txt` entry: that is the cue to check the route's
parameters against the new method and delete the entry.

## Building from source

```bash
git clone <this-repo>
cd signal-cli-api
cargo build --release
# Binary at target/release/signal-cli-api (~4 MB)
```

## Tests

```bash
cargo test   # 267 tests, no Signal account needed
```

## License

MIT
