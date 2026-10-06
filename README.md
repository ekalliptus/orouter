<div align="center">
  <img src="./images/9router.png" alt="ORouter" width="640"/>

  # ORouter

  **Personal AI router — one Node process, one SQLite file, every model behind one local endpoint.**

  Fork of [9Router](https://github.com/decolua/9router) (v0.5.95) with custom features,
  provider fixes, and a production deployment setup.

</div>

---

## What ORouter changes vs upstream

**API keys & policies**

- Per-key **token limit** — lifetime prompt+completion budget, enforced per request (`403` when exhausted)
- Per-key **expiry** (`expiresAt`) with calendar picker, presets, and revive flow
- Per-key **device binding** (`maxDevices` + bound-device chips, unbind/reset) and **model allowlist**
- Per-key **usage dashboard** (click a key) + 24h usage inline under each key

**Providers & engine**

- Antigravity: Claude 5.5 / Gemini 3.7 & 3.8 per-tier model keys, client fingerprint 3.2.1,
  tool-schema hardening (one malformed client tool no longer fails the whole request),
  account fallback with 2-minute model locks
- SeekAI provider (`sa/…`)
- Usage tracking fix for OpenAI-format upstreams (`stream:true` + `include_usage` injection)

**Dashboard**

- "Signal Console" dark redesign (ink-navy + signal green), console-strip headers, tabular stats

**Removed vs upstream**

- CLI Tools, Proxy Pools, and Remote UI pages (routes still exist, navigation only)
- freebuff provider

## Architecture

```
CLI tools (Claude Code, ZCode, Codex, Cursor, …)
        │  http://localhost:20128/v1
        ▼
custom-server.js ── Next.js app ── dashboard + REST API (/api/*)
        │
        ▼
open-sse engine
  handlers/      chat, fetch, embeddings, image, tts/stt …
  translator/    OpenAI ↔ Anthropic ↔ Gemini ↔ … format translation
  providers/     registry (one file per provider) + capabilities
  executors/     transport, auth refresh, retries, account fallback
        │
        ▼
SQLite  (%APPDATA%/9router/db/data.sqlite, additive auto-migrations)
```

## Quick start

```bash
npm install
npm run build
node custom-server.js --port 20128
```

Dashboard: `http://localhost:20128` (set a password on first login; enable
`requireApiKey` in Settings before exposing anything beyond localhost).

Data directory: `%APPDATA%/9router` on Windows, `~/.9router` elsewhere
(override with `DATA_DIR`). Delete the SQLite file to reset; migrations and
legacy JSON import run automatically on boot.

## Using the API

| Endpoint | Format |
|---|---|
| `POST /v1/chat/completions` | OpenAI |
| `POST /v1/messages` | Anthropic |
| `GET /v1/models` | model list |
| `/api/keys` (CRUD), `/api/keys/[id]/usage` | dashboard auth required |

Models are addressed as `provider/model` — for example:

```
ag/gemini-3.8-flash-high        Antigravity, tier is part of the model id
ag/claude-opus-5-5-medium       Claude 5.5 via Antigravity (per-account entitlement)
sa/glm-5.3-flash                SeekAI
glm/glm-5.3                     z.ai
openrouter/openai/gpt-4o-mini   OpenRouter passthrough
```

Thinking/effort levels ride on the model id as a suffix where the provider
supports it: `model(high)`, `model(low)`, `model(8192)`.

## API key policies

| Field | Meaning |
|---|---|
| `maxDevices` | Distinct machines allowed; `0` = unlimited. Devices self-bind on first use. |
| `allowedModels` | Allowlist; empty = all models. Violations get `403` with the allowed list. |
| `expiresAt` | ISO date; expired keys are rejected `401` regardless of `isActive`. |
| `tokenLimit` | Lifetime prompt+completion budget from retained usage history; `0` = unlimited. |

Enforcement runs in `src/sse/services/auth.js` (`enforceKeyPolicy`) on every
routed request. When a provider account fails (quota, 404, auth), the engine
locks that account+model pair for a cooldown and falls through to the next
account.

## Deployment

The public deployment is a single VPS behind a reverse proxy:

- **systemd** unit runs `node custom-server.js --port 20128` (`Restart=always`)
- **Caddy** terminates TLS: SSE needs `flush_interval -1`; add a
  `request_body max_size` and immutable caching for `/_next/static`
- **Cloudflare** proxies the domain: SSL mode *Full (strict)*, always-HTTPS,
  a rate-limit rule on `/v1/*`; the origin firewall only accepts Cloudflare IP
  ranges (a weekly cron can sync `cloudflare.com/ips-v4`/`ips-v6`). Do **not**
  enable Under Attack Mode or Bot Fight Mode — JS challenges break API clients.
- Direct access to the app port is firewalled off; everything goes through the
  domain.
- Deploy flow: sync changed files to the server, `npm run build`, restart the
  service. Version strings are baked into the build — always rebuild after a
  version bump.

## Development notes

- `npm run build` before `node custom-server.js` — the server refuses to start
  from a missing production build.
- The standalone build reads the `PORT` env var, not `--port` (custom-server
  fallback handles the repo-root case).
- After rebuilding, restart the server — the old process serves stale
  hashed assets.
- DB schema changes: add columns/tables to `src/lib/db/schema.js` (additive
  auto-sync applies them on boot) and bump `SCHEMA_VERSION` so a pre-change
  backup is taken.
- Do not add `"type": "module"` to package.json — `custom-server.js` is CJS.
- Watch out with `npx asar extract-file`: it writes the extracted file into
  the current working directory.

## CLI

`cli/` ships the `9router` CLI: connect a CLI tool to a router instance and
manage keys and models. See [cli/README.md](./cli/README.md).

## Acknowledgments

- [9Router](https://github.com/decolua/9router) — the upstream this fork builds on
- [RTK](https://github.com/rtk-ai/rtk) — token-saver compression pipeline ported to JS
- [Caveman](https://github.com/JuliusBrussee/caveman) and
  [Ponytail](https://github.com/DietrichGebert/ponytail) — prompt strategies
  used by the token saver

## License

MIT — see [LICENSE](./LICENSE).
