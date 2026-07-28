# Plan: Redesign Neobrutalism (global theme + logo) — tanpa rusak fitur

## Tujuan (dari keputusan lo)
Neobrutalism beneran: corner tajam dimana-mana, border tebal hitam, shadow kotak keras, warna flat kontras, favicon "O" neobrutal, logo in-app diupdate konsisten.

## Bukti arsitektur (audit membuktikan bisa dilakukan aman)
Repo lo pakai **Tailwind v4 CSS-first**, semua warna lewat CSS custom properties di `globals.css` (`:root` light + `.dark` dark), di-map ke utility via `@theme inline` (globals.css:173-236). Override token otomatis mengalir ke `bg-primary`, `bg-surface`, `text-text`, `border-border`, dll (dipakai ratusan kali). Untuk ciri neobrutal yang GAK lewat token (corner radius 563 `rounded-*`, border width 1px), gue pakai **global CSS rules** (element selector) — bukan menyapu 106 file (berisiko).

## Perubahan (4 area, dipisah supaya bisa diuji/rollback)

### Area 1 — Neobrutalism palette + shadow (globals.css `:root` + `.dark`)
Override token warna jadi neobrutal: background krem/off-white (#F4EDE0), surface putih bersih, primary tetap oranye TAPI flat solid (bukan gradient), accent kontras tinggi (hitam tebal + kuning/cyan pop). Shadow tokens (`--shadow-soft/warm/elevated/elev`) → **hard offset shadow** neobrutal (mis. `4px 4px 0 #000`, no blur). Border token `--color-border` → hitam solid. Semua mengalir otomatis ke utility yg sudah dipakai.
- Light: warm cream bg, hard black borders, flat accent.
- Dark: deep charcoal, same hard black borders, flat accent.

### Area 2 — Global neobrutalist CSS rules (globals.css, blok baru)
Ciri neobrutal yang GAK lewat token, diselesaikan via rules global (bukan menyapu file):
- **Corner tajam**: zero radius default di surface/card/input/button/select, TAPI **preserve exceptions** — `rounded-full` Badge, Toggle switch, avatar, status pill tetap bulat (dikecualikan via `:where()` selector, supaya gak rusak elemen interaktif).
- **Border tebal**: border default jadi `2px solid black` di surface/card/button, dengan class override (`rounded-full`, dll) tetap dihormati.
- **Focus ring neobrutal**: global `:focus-visible` jadi thick black outline offset (bukan ring lembut).
- `--radius-brand` → 0 (cards jadi tajam).

### Area 3 — Favicon + PWA icon "O" neobrutal (file baru/replace)
Bikin SVG baru: huruf "O" tebal, border hitam 3px, hard offset shadow, flat color. Beda dari "9" asli tapi similar vibe (letter-mark di kotak). File:
- Replace `public/favicon.svg` (dipakai metadata + ProviderTopology:113)
- Replace `public/icons/icon-192.svg`, `public/icons/icon-512.svg` (PWA manifest)
- Replace `src/app/favicon.ico` (fallback, rasterize dari SVG baru)
- Cek `manifest.js` theme_color match palette baru

### Area 4 — Logo in-app neobrutal (Sidebar + Landing Navigation)
Update `Sidebar.js:120-131` dan `landing/components/Navigation.js:19-22`: ganti icon `hub` + gradient box → pakai SVG "O" baru (atau styled box neobrutal: border tebal, hard shadow, flat). Wordmark "ORouter" tetap (typography lebih chunky via font-weight). Konsisten dengan favicon.

## Catatan honest soal "agresif radius"
Lo pilih agresif `border-radius:0` dimana-mana. Gue implementasikan dengan **exception list** (Toggle, Badge, avatar, status pill tetap bulat) supaya gak rusak UX. Lo akan lihat preview sebelum deploy — kalau ada elemen yg terlihat aneh jadi kotak, gue tweak exception list. Ini reversible (CSS doang).

## Yang TIDAK diubah (jaga fitur)
- Material Symbols font (berisiko, gak perlu — neobrutal dicapai via CSS).
- ProviderIcon.js (logo provider, beda sistem).
- Semua logic/API/fitur.
- `tailwind.config` (gak ada — Tailwind v4 css-first).

## Deliverable: preview dulu sebelum deploy
Gue bikin perubahan di lokal, build lokal (Mac, RAM kuat), lalu kasih lo **screenshot/preview** vibe neobrutal-nya. Lo approve baru deploy ke VPS. Bukan langsung push ke production.

## Verifikasi
- Build lokal EXIT 0 (no compile error).
- Visual: buka dashboard lokal → lihat palette/shadow/border neobrutal + favicon baru.
- Deploy ke VPS (stop app → build → start) setelah lo approve preview.
- Test: navigate dashboard, pastikan Toggle/Badge/avatar tetap normal (exception list jalan).

## Rujukan path opsi-2 (per-komponen) buat nanti
Buat neobrutalism yg lebih halus per-komponen (mis. rounded-lg → rounded-none selectively), titik mulainya: 563 `rounded-*` di 106 file, 205 hardcoded hex. Itu bisa gue kerjakan terpisah kalau lo mau refine setelah lihat preview global.