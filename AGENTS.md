# Magnolia — Agent Instructions

Tauri 2 + Svelte 4 torrent streaming desktop app with embedded libmpv for playback. See [README.md](README.md) for a project overview.

## Build & Dev Commands

```bash
npm install           # install JS dependencies (pnpm/npm both work)
npm run tauri:dev     # dev mode: Vite (localhost:1420) + Rust backend
npm run tauri:build   # production build: vite build + cargo tauri build
npm run dev           # Vite only (no Tauri shell)
```

Rust is in `src-tauri/`. Run `cargo check` from `src-tauri/` to validate Rust changes without a full build.

## Architecture

**No frontend router.** Navigation is a state machine in `src/App.svelte` via boolean/object flags (`selectedMedia`, `showVideoPlayer`, `viewAllData`). Do not introduce a router.

**Layers:**
- `src/lib/` — Svelte components + utility JS
- `src/lib/stores/` — Svelte writable stores (persisted to `localStorage` or Rust backend)
- `src/styles/` — per-feature CSS files, imported in `main.js`
- `src-tauri/src/` — Rust: Tauri commands, mpv FFI, torrent engine, scrapers

**Cross-component events** use DOM `window.dispatchEvent` (not stores): `openMediaDetail`, `openVideoPlayer`, `viewAll`, `updateTitleBarColor`, `videoControlsVisibility`, `settingsChanged`.

**Tauri IPC:**
- JS → Rust: `invoke('command_name', { camelCaseParams })` — see full list in `src-tauri/src/main.rs`
- Rust → JS: `listen('mpv-progress-update' | 'mpv-end-file' | 'mpv-tracks-loaded', cb)` in `VideoPlayer.svelte`

## Conventions

| Area | Convention |
|---|---|
| Svelte components | `PascalCase.svelte` |
| CSS classes | `kebab-case` |
| Rust command params | `snake_case` in Rust, `camelCase` in JS |
| Stores | `camelCaseStore.js` in `src/lib/stores/` |
| CSS | One file per feature in `src/styles/`; attach via `@import` in the component `<style>` block |
| Theming | CSS custom properties (defined in `src/styles/app.css`) |

## Key Files

- `src/App.svelte` — root state machine, navigation history stack, modal gating, DOM event bus
- `src/lib/VideoPlayer.svelte` — mpv UI shell; listens to mpv events; polls torrent status at 1s intervals
- `src/lib/MediaDetail.svelte` — TMDB detail view; triggers torrent search + playback flow
- `src/lib/tmdb.js` — all TMDB API calls; token fetched at runtime from a Netlify proxy (no key in repo)
- `src-tauri/src/main.rs` — all `#[tauri::command]` handlers + app state registration
- `src-tauri/src/mpv/` — libmpv FFI (`ffi.rs`), safe wrapper (`handle.rs`), OS embedding (`embed.rs`), event loop (`event_loop.rs`)
- `src-tauri/src/torrent.rs` — librqbit torrent session + axum HTTP server for range-request streaming
- `src-tauri/src/search/` — per-site scrapers: `nyaa.rs`, `piratebay.rs`, `eztv.rs`, `limetorrents.rs`

## Pitfalls

- **One mpv instance protected by a `Mutex`** — all mpv commands serialize; avoid holding the lock in async contexts.
- **`wipe_all_torrent_files` on every stream open** — torrent data is wiped before each playback session; never assume previously added torrents persist.
- **Video player remount uses a 50ms `setTimeout`** in `App.svelte` — code in `VideoPlayer.svelte` `onMount`/`onDestroy` must be idempotent.
- **`WatchHistoryItem` dual casing** — fields are `snake_case` in Rust but the JS store maps them to both formats; keep both in sync when modifying.
- **TMDB token via external proxy** — if `magnolia-tmdb.netlify.app` is unreachable, all metadata calls silently fail (no retry logic).
- **CSP is `null`** — no Content Security Policy; never render user-controlled HTML.
- **macOS-only features** — `macOSPrivateApi: true` enables vibrancy/transparency; test UI changes on target platform.
- **Tracker selection lives in `search_nyaa_filtered`** — despite the name, this function in `main.rs` handles all trackers and the auto/manual/anime fallback logic.
