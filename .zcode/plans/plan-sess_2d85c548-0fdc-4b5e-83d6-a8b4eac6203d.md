# Plan: Tambah Card Setup Kimi CLI di Dashboard CLI Tools

## Tujuan (dari keputusan lo)
Tambah card full-feature buat setup Kimi Code CLI client biar point ke ORouter. Mirror pattern DeepSeek TUI (auto-write config TOML, status badge real, Apply/Reset/Manual buttons). Dukung kedua config path (`~/.kimi-code/` & `~/.kimi-cli/`).

## Bukti investigasi (pattern match akurat)
- **Kimi Code CLI beneran ada** (Moonshot AI, npm `@moonshot-ai/kimi-code`). Config: `~/.kimi-code/config.toml`, section `[providers.<name>]` dgn `type/base_url/api_key/model`. **Skema nyaris identik dgn DeepSeek TUI** → mirror tepat.
- Image `kimi.png` + `kimi-coding.png` **sudah ada** di `public/providers/` → gak perlu aset baru.
- DeepSeekTuiToolCard + backend route = template ideal (single config.toml, TOML parse/write, install-detect, has9Router detection).
- ToolDetailClient switch (line 117-146) dispatch toolId→card. all-statuses STATUS_GETTERS (line 30) buat badge real.

## File yg akan saya ubah/buat (5 file)

### 1. `src/shared/constants/cliTools.js` — add entry (1 edit)
Tambah entry `kimi` setelah `deepseek-tui` (line ~339). configType:"custom". Mirror field DeepSeek:
```js
kimi: {
  id: "kimi",
  name: "Kimi CLI",
  image: "/providers/kimi-coding.png",
  color: "#1E3A8A",
  description: "Moonshot AI Kimi Code CLI — terminal coding agent",
  docsUrl: "https://moonshotai.github.io/kimi-cli/",
  configType: "custom",
  defaultCommand: "kimi",
  modelAliases: ["kimi-k3", "k3", "kimi-for-coding", "kimi-k2.7-code", "kimi-k2.6"],
  defaultModels: [
    { id: "kimi-k3", name: "Kimi K3", alias: "kimi-k3" },
    { id: "k3", name: "Kimi K3 (Code)", alias: "k3" },
    { id: "kimi-for-coding", name: "Kimi for Coding", alias: "kimi-for-coding" },
    { id: "kimi-k2.7-code", name: "Kimi K2.7 Code", alias: "kimi-k2.7-code" },
    { id: "kimi-k2.6", name: "Kimi K2.6", alias: "kimi-k2.6" },
  ],
  notes: [
    { type: "info", text: "Kimi Code CLI uses ~/.kimi-code/config.toml (also checks ~/.kimi-cli/config.toml). 9Router registers as an OpenAI-compatible provider." },
    { type: "warning", text: "Install: npm i -g @moonshot-ai/kimi-code OR curl -LsSf https://code.kimi.com/install.sh | bash" },
  ],
},
```

### 2. `src/app/api/cli-tools/kimi-settings/route.js` — NEW (mirror deepseek backend)
GET/POST/DELETE. Beda dgn DeepSeek:
- **Cek 2 config path** (`~/.kimi-code/config.toml` dulu, fallback `~/.kimi-cli/config.toml`) — dukung keduanya sesuai pilihan lo.
- Install-detect: `which kimi` (npm global) ATAU config file ada.
- TOML schema Kimi: section `[providers.9router]` dgn `type="openai"`, `base_url`, `api_key`, `model` (beda dikit dgn deepseek yg pake top-level `provider="openai"` + `[providers.openai]`). Saya pakai schema yg verified dari Kimi docs (`type`/`base_url`/`api_key`/`model`).
- has9Router: cek `providers.9router.base_url` match localhost/tunnel.
- POST tulis config ke path yg ketemuan (prefer `.kimi-code`), atau default `.kimi-code` kalau gak ada.
- DELETE: hapus section `[providers.9router]` (preserve config lain user).

### 3. `src/app/(dashboard)/dashboard/cli-tools/components/KimiToolCard.js` — NEW (mirror DeepSeekTuiToolCard)
Copy DeepSeekTuiToolCard structure (~14KB), adaptasi:
- ENDPOINT → `/api/cli-tools/kimi-settings`
- Section key `providers.9router` (bukan `providers.openai`)
- Status detection baca `providers.9router.base_url`
- Branding: "Kimi Code CLI", install hint npm/curl, image `/providers/kimi-coding.png`
- Pakai komponen yg sama: BaseUrlSelect, ApiKeySelect, ModelSelectModal, ManualConfigModal, matchKnownEndpoint
- Model list: kimi-k3/k3/kimi-for-coding/k2.7-code/k2.6 (dari defaultModels)

### 4. `src/app/(dashboard)/dashboard/cli-tools/components/index.js` — add export (1 line)
`export { default as KimiToolCard } from "./KimiToolCard";`

### 5. `src/app/(dashboard)/dashboard/cli-tools/[toolId]/ToolDetailClient.js` — add case (2 edit)
- Import KimiToolCard (line 8-13 area)
- `case "kimi": return <KimiToolCard {...commonProps} ... />;` (line ~138, sebelum jcode)
- Register status getter di `src/app/api/cli-tools/all-statuses/route.js` line 30: `"kimi": kimiGet`

## Yang TIDAK saya ubah
- Provider registry Kimi (`open-sse/providers/registry/kimi.js`) — foundation udah ada, gak usah sentuh.
- OAuth Kimi flow — terpisah (upstream provider), bukan bagian CLI tool card.
- Card lain — gak ikut.

## Risk & mitigasi
- **TOML schema Kimi** — agent nemuin field `type`/`base_url`/`api_key`/`model` dari third-party docs (Moonshot official docs tipis). Saya pakai schema paling umum + fallback baca config existing user (gak overwrite section lain). Kalau ternyata field beda, user tetap bisa pakai "Manual Config" modal (copy-paste).
- **2 config path** — saya prefer `.kimi-code` (tool baru), fallback detect `.kimi-cli`. Aman.
- **No breaking change** — entry baru doang, gak modify existing.

## Verifikasi
- `node --check` semua file JS baru/diubah.
- Build lokal (Mac, RAM kuat) sebelum deploy.
- Manual test: navigasi `/dashboard/cli-tools` → tile "Kimi CLI" muncul → click → card render dgn status + Apply/Reset/Manual buttons.
- Deploy ke VPS (stop app → build → start, SKIP_BUILD=1).

## Catatan honest
- Card ini buat **config client Kimi CLI user** point ke ORouter. Bukan nambah provider Kimi upstream (itu udah ada). Jadi card cuma bantu user set `~/.kimi-code/config.toml` biar kimi CLI-nya jalan lewat ORouter (dgn fallback ke provider lain pas kimi 429, etc).
- Status badge "Connected/Not installed" cuma akurat kalau user akses dashboard dari mesin yg sama dgn Kimi CLI-nya (karena route baca `~/.kimi-code/config.toml` lokal). Ini limitasi arsitektur yg sama dgn card lain (DeepSeek/jcode) — dashboard di VPS gak bisa baca config di mesin user. DefaultToolCard/guide card bebas limitasi ini tapi gak auto-write.