# Plan: Fix dashboard "kurang smooth" — polling & main-thread contention

## Root cause (audit membuktikan, bukan tebak)

Gejala lo (scroll stutter, click/input delay, data update nge-lag) = **main thread contention**, BUKAN bundle JS (semua heavy dep sudah lazy) dan BUKAN list rendering (semua list sudah dibatesin/paginate). Penyebabnya: **client polling numpuk & gak respect tab visibility**. Audit nemuin:

- **`RequestLogger.js:18`** polling HTTP tiap **3 detik**, always-on walau tab hidden, refetch full table + re-render tiap tick. Worst offender.
- **10 komponen gak ada `visibilitychange` guard** → polling terus jalan walau lo ganti tab ke lain (bikin VPS & browser sibuk percuma).
- **Banyak clock 1Hz independen** (UsageStats, CooldownTimer per-row, ConnectionRow per-row) — gak fetch tapi trigger re-render. Harusnya 1 tick shared.

Bundle audit: Monaco/recharts/@xyflow/marked **sudah lazy** → bukan masalah. Skip code-splitting (sudah rapi).

## Perubahan kode (3 area, low-risk, additive)

### Fix 1 — Pause polling saat tab hidden (impact tertinggi)
Bikin helper hook `useVisibilityAwarePolling` (atau minimal: guard existing intervals). Saat `document.hidden`, polling/clock pause otomatis; resume saat tab visible lagi. Diterapin di:
- `src/shared/components/RequestLogger.js:18` (3s fetch — paling penting)
- `src/app/(dashboard)/dashboard/providers/components/ModelAvailabilityBadge.js:45` (30s fetch)
- `src/shared/components/UsageStats.js:45` useClock (1s re-render)

Effect: pas lo ganti tab/buka halaman lain, beban main thread drop drastis. Langsung ngaruh ke "klik/scroll smooth".

### Fix 2 — Lambatkan RequestLogger & jadikan on-demand
- `RequestLogger.js:18`: 3s → **5s** (cukup buat "real-time feel", setengah beban).
- Tambahan: skip refetch jika response identik (ETag/signature dedup, pola yg udah dipake di UsageStats SSE line 301). Hindari re-render kalo data gak berubah.

Effect: beban network + re-render berkurang 40-60% tanpa kehilangan feel real-time.

### Fix 3 — Share 1 tick global buat clock 1Hz
Saat ini: tiap CooldownTimer/ConnectionRow bikin `setInterval` sendiri. Kalau lo punya 10 row cooldown = 10 timer 1Hz = 10 re-render/detik. Bikin **1 global tick** (via Zustand store yg udah ada / context) yg semua komponen subscribe. 

Files terkait: `CooldownTimer.js`, `ConnectionRow.js`, `UsageStats.js` useClock.

Effect: dari N timer → 1 timer. Re-render coalesced.

## Yang TIDAK saya ubah (sudah dipertimbangkan & ditolak via audit)
- **Virtualisasi list** — audit buktiin semua list bounded (20-200 rows). Minim impact, sia-sia.
- **Code-split Monaco/recharts** — udah lazy semua. Bukan masalah.
- **Server-side** — udah gue fix kemarin (DB prune, headroom mati, cache TTL).

## Risk & cara mitigasi
- Semua perubahan **additive** (guard + dedup), gak ubah API/data.
- `useVisibilityAwarePolling` pakai pola standar `visibilitychange` + cleanup di useEffect return.
- Fix 2 (signature dedup) replikasi pola yg udah terbukti jalan di `UsageStats.js:301`.

## Verifikasi
- Build lokal (Mac, RAM kuat) → deploy ke VPS via cara yg sama kemarin (stop app → build → start). **ATAU** build lokal + scp kalo lo mau hindari downtime.
- Manual: buka dashboard, scroll/navigate, ganti tab → verify smooth + polling pause (cek Network tab browser: request berhenti saat tab hidden).
- Tidak ada test JS existing yg harus jalan (perubahan client-only).

## Catatan
Plan ini fokus **client-side rendering perf** (yang lo rasain "kurang smooth"). Kalau setelah ini masih ada gejala, kemungkinan besar sisa-nya = latensi jaringan ke VPS lo (RTT ke region provider) — itu gak bisa di-fix dari kode, cuma infrastruktur (CDN/region).