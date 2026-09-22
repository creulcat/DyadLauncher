# Network hosts

Every host Dyad Launcher's desktop app (`apps/app`, `apps/app-frontend`, `packages/app-lib`) can
contact over the network, and why. This is goal 7, item 6 of [GOALS.md](GOALS.md) — the follow-up
to item 7's guard script (item 7), which fails CI when a hostname shows up in source, config or the
CSP that isn't listed here.

Compiled by reading every `reqwest`/`fetch`/`invoke` call site in `packages/app-lib`, `apps/app` and
`apps/app-frontend` (2026-09-22), cross-checked against the CSP allowlist in
`apps/app/tauri.conf.json`, and against real traffic in the webview's own DevTools (Ctrl+Shift+I) —
per the decided approach, this only shows the frontend's own requests; everything the Rust side sends
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
| `api.mclo.gs` | mclogs: crash analysis (`client.mclogs.insights_v1.analyse`, sends the game's log content) and log paste/sharing. | **Not opt-in the way GOALS.md previously assumed.** Crash analysis (`analyseForCrash()` in `pages/instance/logs/index.vue`) runs automatically whenever the Logs page is opened for a non-running instance, and automatically again whenever a game process finishes — with no confirmation prompt. Only the separate "share this log" action is an explicit click. See "Findings from this pass" below. |
| `modrinth.com` / `*.modrinth.com` | CSP wildcard covering the above plus any other `*.modrinth.com` subdomain a project description or API response might link to. | As needed for the above. |
| `discord.com` | `frame-src` only — lets a project description embed a Discord invite widget. Passive: the iframe only loads if a project's own description embeds one. | Only when viewing a project whose description embeds a Discord widget. |
| `*.githubusercontent.com` | `media-src` only — lets a project description embed a `<video>`/`<audio>` clip hosted on GitHub's raw-content CDN, the same "arbitrary remote content in project descriptions" tradeoff as the `img-src https:` wildcard (see GOALS.md goal 7). | Only when viewing a project whose description embeds such a clip. |

## Mojang / Microsoft (Minecraft account and game files)

| Host | Reason | When |
|---|---|---|
| `login.live.com`, `*.xboxlive.com` (`auth`, `device.auth`, `user.auth`, `sisu`, `xsts`), `xboxlive.com` | Microsoft/Xbox OAuth device-code flow for signing in a Minecraft account. This is the Minecraft account needed to launch the game, unrelated to the Modrinth account system goal 3 removed. | Only when signing in a Minecraft account, or silently refreshing an existing session before launch. |
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

- **`fill.papermc.io` and `api.purpurmc.org` were dead CSP entries, now removed.** They're part of
  `packages/api-client`'s always-present `paper`/`purpur` client modules (used by the website's server
  hosting features), but zero code in `apps/app-frontend` or `packages/app-lib` ever calls into them —
  the same "allowed but never actually reachable" pattern goal 7 already found and removed for Archon
  and shared-instances. The previous audit had listed these as a deliberately-kept live feature; that
  was wrong, and the CSP entries are gone as of this pass.
- **mclo.gs crash analysis is not opt-in.** The previous audit's "mclo.gs log sharing (only when the
  user clicks it)" undersold it: analysis fires automatically on two triggers (opening the Logs page
  for a stopped instance, and a game process finishing), sending log content to `api.mclo.gs` with no
  confirmation prompt. Only the separate "share" action is an explicit click. This wasn't changed as
  part of this pass — item 6 is about documenting current behavior, not changing it — but it's worth a
  deliberate decision (leave as-is, add a setting, or gate behind confirmation) rather than carrying
  the incorrect assumption forward.
- `config.ts`'s unused `siteUrl` field (and the `MODRINTH_URL` env var feeding it) were dead — nothing
  ever read `config.siteUrl` (the one place that needed a "link to modrinth.com" already hardcodes the
  literal string instead). Removed along with the CSP fixes above.
- Excluded from the tables above: URLs that appear only in code comments or doc-links (source
  attribution for bundled default skins, PNG/XML spec references, Apple's plist DTD), and hrefs in
  error-message text that open in the user's own system browser rather than being fetched by the app
  (the Xbox/Microsoft support links in `minecraft-auth-errors.ts`) — the app itself never contacts
  these.
