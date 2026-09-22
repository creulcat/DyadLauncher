# Network hosts

Every host Dyad Launcher's desktop app (`apps/app`, `apps/app-frontend`, `packages/app-lib`) can
contact over the network, and why. This is goal 7, item 6 of [GOALS.md](GOALS.md) — the source of
truth item 7's CI guard script (`scripts/check-network-allowlist.ts`) checks against, failing CI
when a hostname shows up in source, config or the CSP that isn't listed here.

Compiled by reading every `reqwest`/`fetch`/`invoke` call site in `packages/app-lib`, `apps/app` and
`apps/app-frontend` (2026-09-22), cross-checked against **two** separate allowlists — the webview CSP
in `apps/app/tauri.conf.json` (what the webview's own `fetch`/`<img>`/`<iframe>`/etc. may load) and
the `http:default` permission scope in `apps/app/capabilities/plugins.json` (what
`@tauri-apps/plugin-http`'s `fetch` may target — used by `packages/api-client`'s Tauri platform for
every `TauriModrinthClient` request, i.e. all of the frontend's Modrinth/mclogs API calls, not just
the version check) — and against real traffic in the webview's own DevTools (Ctrl+Shift+I) per the
decided approach, this only shows the frontend's own requests; everything the Rust side sends
(downloads, `launcher-meta`, the GitHub version check, Discord, Java and jar downloads) is covered by
reading the `reqwest` call sites instead. This list is a snapshot, not a promise — a host built
entirely at runtime from an API response (e.g. a mod's own download URL) won't show up as a literal
string anywhere, and the CI guard in item 7 can only catch hardcoded hostnames, not those.

## Modrinth-operated

| Host | Reason | When |
|---|---|---|
| `api.modrinth.com` | The Labrinth API: search, browse, project/version/user metadata, content resolution. The actual point of the launcher (goal 3 explicitly keeps anonymous browsing/search/downloads). | Every browse/search/project-view action, and whenever installed content needs metadata refreshed. Always on, no toggle — this is core functionality, not tracking. |
| `cdn.modrinth.com` | The content CDN: mod/resource-pack/modpack/shader file downloads, project icons. Also one hardcoded fallback (a placeholder gallery-image banner in `project/Gallery.vue`). | Whenever content is downloaded or a project's images are shown. Always on, same as above. |
| `cdn-raw.modrinth.com` | Static assets: the Inter UI font (`packages/assets/styles/inter.scss`) and the Minecraft font (`apps/app-frontend/src/assets/stylesheets/global.scss`). Decided 2026-09-21 (goal 7) to keep bundling these from Modrinth's CDN rather than vendoring them — the Minecraft font is Mojang-derived and this repo doesn't want to redistribute it. | Loaded on every app start (CSS `@font-face`). Always on. |
| `launcher-files.modrinth.com` | Fallback avatar image (`AccountsCard.vue`'s default head, used when no local skin render is available) and the fallback content-card image in a couple of older-style cards. | Only when a local render isn't available. Not user-toggleable, but no longer the primary path since goal 7 item 2 moved account avatars to a local render first. |
| `launcher-meta.modrinth.com` | Modrinth's mirror of the Minecraft and mod-loader (Fabric/Forge/Quilt/NeoForge) version manifests, needed to create and launch instances. Decided 2026-09-21 (goal 7): kept and documented — replacing it means talking to Mojang/Fabric/Forge/Quilt/NeoForge directly, a large separate project, and this is not tracking. | Whenever instance creation or launch needs version metadata. Always on, core functionality. |
| `api.mclo.gs` | mclogs: log paste/sharing (`client.mclogs.logs_v1.create`, sends the game's log content). | **Opt-in only, as of 2026-09-22.** The automatic crash-analysis call (`insights_v1.analyse`, fired without confirmation on opening the Logs page or a game process finishing) was removed — see "Findings from this pass" below. The only remaining path to this host is the explicit "Share" button on the Logs page. |
| `modrinth.com` / `*.modrinth.com` | CSP wildcard covering the above plus any other `*.modrinth.com` subdomain a project description or API response might link to. | As needed for the above. |
| `discord.com` | `frame-src` only — lets a project description embed a Discord invite widget. Passive: the iframe only loads if a project's own description embeds one. | Only when viewing a project whose description embeds a Discord widget. |
| `*.githubusercontent.com` | `media-src` only — lets a project description embed a `<video>`/`<audio>` clip hosted on GitHub's raw-content CDN, the same "arbitrary remote content in project descriptions" tradeoff as the `img-src https:` wildcard (see GOALS.md goal 7). | Only when viewing a project whose description embeds such a clip. |

## Mojang / Microsoft (Minecraft account and game files)

| Host | Reason | When |
|---|---|---|
| `login.live.com`, `xboxlive.com`, `auth.xboxlive.com`, `device.auth.xboxlive.com`, `user.auth.xboxlive.com`, `sisu.xboxlive.com`, `xsts.auth.xboxlive.com` | Microsoft/Xbox OAuth device-code flow for signing in a Minecraft account. This is the Minecraft account needed to launch the game, unrelated to the Modrinth account system goal 3 removed. | Only when signing in a Minecraft account, or silently refreshing an existing session before launch. |
| `api.minecraftservices.com` | Minecraft profile/entitlement/skin API (own profile, skins, capes). | On login and periodic profile refresh. |
| `sessionserver.mojang.com` | Mojang session-server profile lookups (used for other accounts' public profile data, e.g. rendering skins). | As needed when resolving another account's profile. |
| `textures.minecraft.net` | Mojang's skin/cape texture CDN. | Whenever a skin/cape texture is displayed and can't be rendered from an already-local source. |
| `libraries.minecraft.net`, `resources.download.minecraft.net` | Game libraries and assets needed to launch Minecraft itself. | Whenever an instance needs a library/asset it doesn't already have cached. |

None of the above are toggleable — they're required for the "launch Minecraft" and "sign in a
Minecraft account" features, which are the launcher's job, not tracking.

## Third-party services

| Host | Reason | When |
|---|---|---|
| `api.azul.com` | Azul Zulu JRE metadata/download API, used for automatic Java runtime management. | Whenever an instance needs a Java runtime the launcher doesn't already have. |
| `github.com` | The version check (goal 4): fetches `github.com/creulcat/DyadLauncher/releases/latest/download/updates.json`, and the manual/auto "Download"/"Check for updates" actions open a `github.com` releases page or installer link in the system browser. | **Toggleable as of goal 7 item 5** — Settings → Behavior → "Check for updates automatically" (on by default) gates the launch-time and hourly automatic checks. The manual "Check for updates" button in the Settings footer always works regardless of the toggle, since that's a deliberate click. |
| `www.youtube.com`, `www.youtube-nocookie.com` | `frame-src` only — lets a project description embed a YouTube trailer/showcase video. Verified live 2026-09-22 against `cobblemon-fabric`'s real embedded trailer after the CSP `frame-src` trim. | Only when viewing a project whose description embeds a YouTube video. |

## Local only, not a network host

| What | Detail |
|---|---|
| Discord Rich Presence | Connects over a local IPC socket/named pipe to the Discord desktop client (`discord-rich-presence` crate, `DiscordIpcClient`), not a direct HTTPS request to `discord.com`. Opt-in, off by default (goal 3). |

## Findings from this pass (2026-09-22)

- **A second, separate allowlist had the same dead hosts the CSP did, and had one CSP never caught:
  `apps/app/capabilities/plugins.json`'s `http:default` permission scope.** This gates
  `@tauri-apps/plugin-http`'s `fetch`, which is what `packages/api-client`'s Tauri platform actually
  uses for every `TauriModrinthClient` request — the CSP alone doesn't cover this, since plugin-http
  calls don't go through the webview's own fetch. It still allowed `https://*.nodes.modrinth.com/*`
  and `http://*.taila228c5.ts.net`/`https://*.taila228c5.ts.net` (the same Tailscale-dev-node/hosting
  leftovers goal 7 item 1 already removed from the CSP, apparently missed there) and
  `https://fill.papermc.io/*`/`https://api.purpurmc.org/*` (see next finding) — all removed now. The
  `github.com/creulcat/DyadLauncher/releases/*`, `modrinth.com`/`*.modrinth.com`, `api.mclo.gs`, and
  the two `localhost:8000`/`127.0.0.1:8000` dev-server entries are genuinely used and stay.
- **`fill.papermc.io` and `api.purpurmc.org` were dead in both allowlists, now removed from both.**
  They're part of `packages/api-client`'s always-present `paper`/`purpur` client modules (used by the
  website's server hosting features), but zero code in `apps/app-frontend` or `packages/app-lib` ever
  calls into them — the same "allowed but never actually reachable" pattern goal 7 already found and
  removed for Archon and shared-instances. The previous audit had listed these as a deliberately-kept
  live feature; that was wrong.
- **mclo.gs crash analysis was found to be non-opt-in, then removed (2026-09-22).** The previous
  audit's "mclo.gs log sharing (only when the user clicks it)" undersold it: analysis fired
  automatically on two triggers (opening the Logs page for a stopped instance, and a game process
  finishing), sending log content to `api.mclo.gs` with no confirmation prompt. Decided: this fork
  doesn't want any automatic upload, opt-in or not. `analyseForCrash()` and its two auto-invocations
  were removed from `pages/instance/logs/index.vue`, along with the crash-analysis panel it fed
  (`ConsoleManagerContext.crashAnalysis`/`onDismissCrash`, both optional in the shared type, so the
  panel simply never renders now — `packages/ui`/the website are unaffected). The only remaining path
  to `api.mclo.gs` is the separate "Share" button, unchanged and still an explicit click.
- `config.ts`'s unused `siteUrl` field (and the `MODRINTH_URL` env var feeding it) were dead — nothing
  ever read `config.siteUrl` (the one place that needed a "link to modrinth.com" already hardcodes the
  literal string instead). Removed along with the CSP fixes above.
- **A second hardcoded "Modrinth App" User-Agent, missed by goal 7 item 4.** Item 4 rebranded
  `launcher_user_agent()` (`packages/app-lib/src/lib.rs`), used for Modrinth/general HTTP requests, but
  `minecraft_auth.rs`'s separate `MINECRAFT_SERVICES_USER_AGENT` constant — sent with every request to
  `api.minecraftservices.com` and Mojang's session server for profile/skin/cape operations — still said
  `"Modrinth App (support@modrinth.com; https://modrinth.com/app)"`. Rebranded to match, as part of
  this pass.

## Non-network hostnames that appear in source

These show up as literal text somewhere in the codebase but are never contacted by the app itself —
listed here so the item 7 guard script doesn't flag them as new, undocumented hosts.

| Host | Where it appears | Why it's not a real request |
|---|---|---|
| `www.w3.org` | `background.rs` (writes a literal `xmlns='http://www.w3.org/2000/svg'` into a generated SVG file), doc comments in `png_util.rs` citing the PNG spec | XML namespace value and spec citations, not a fetched URL. |
| `www.apple.com` | `App.entitlements`, `Info.plist`, `apps/app/src/api/shortcuts/macos.rs` | The standard plist `<!DOCTYPE>` declaration every macOS plist file carries, not a fetched URL. |
| `web.archive.org`, `community-content-assets-cms.minecraft.net`, `www.minecraft.net`, `minecraft.wiki` | Doc comments in `default_skins.rs` citing where each bundled default skin's texture originally came from | Source-attribution comments; the skins themselves are bundled in the binary, not downloaded. |
| `support.xbox.com`, `www.xbox.com`, `help.minecraft.net`, `account.microsoft.com` | `<a href>` links inside error-message text in `minecraft-auth-errors.ts` | Opened in the user's own system browser when clicked; the app itself never fetches these. |
| `support.modrinth.com` | The WebView2-corrupted-install error dialog in `apps/app/src/main.rs` | Text of a native dialog, opened in the user's own browser if they type/click it; not fetched by the app. Cosmetic "Modrinth App" wording in this dialog is a known, separately-tracked rebrand item (see GOALS.md goal 7's "Known tradeoffs" note), not part of this goal. |
| `asset.localhost` | `apps/app/src/api/utils.rs`, the asset-protocol `img-src`/`connect-src` CSP entries | Tauri's internal `asset://`/`http://asset.localhost` protocol for serving local files (icons, backgrounds) to the webview — not an external network host. |
| `tauri.localhost` | `App.vue` (checking `target.href` before intercepting link clicks) | Tauri's internal production webview origin, not an external host. |
| `ipc.localhost` | The CSP's `connect-src` | Tauri's internal frontend-to-Rust IPC bridge, not an external host. |
| `localhost`, `127.0.0.1` | `vite.config.ts`'s dev server, `.env.local`'s local-labrinth URLs, the matching entries in `capabilities/plugins.json`'s `http:default` scope | Only reachable when running a local dev build against a locally-run `labrinth`; never a real external host. |
| `vitejs.dev`, `v2.tauri.app` | Doc-comment links in `vite.config.ts` | Comments pointing at Vite/Tauri's own documentation, not a fetched URL. |
| `schema.tauri.app` | The `$schema` key at the top of every `tauri*.conf.json` | Editor tooling (JSON schema validation), not fetched at runtime. |
| `clientauth.one.digicert.com`, `timestamp.sectigo.com`, `timestamp.digicert.com` | `apps/app/tauri-release.conf.json`'s Windows code-signing (`jsign`) configuration | Contacted by the CI signing step (`jsign`) at build time, on Windows tag/release builds only when signing credentials are present (goal 5) — never by the running app. |
