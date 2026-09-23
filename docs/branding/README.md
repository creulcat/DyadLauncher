# Dyad Launcher branding assets

- **`mark.svg`** — the canonical mark: two overlapping diamonds (green `#54ff54` → cyan `#55ffff`
  tile, the overlap drawn as its own darker shape) on a rounded-square tile with transparent
  corners. This is the source of truth; every other icon file in the repo should trace back to it.
- **`mark-glyph.svg`** — the same twin-diamond shape with no tile/background, for contexts that
  composite their own chip (currently: `apps/app/icons/apple.icon`, Apple's Icon Composer format).

## Where this has already been applied directly

- `apps/app/icons/apple.icon/Assets/logo.svg` + `icon.json` (macOS/watchOS layered icon)
- `.idea/icon.svg` (JetBrains project icon)
- `apps/app-frontend/src/assets/welcome/dyad-mark.svg` (in-app welcome screen)
- `apps/app-frontend/src/components/ui/SplashScreen.vue` (app launch splash — inlines the same mark,
  next to plain "Dyad Launcher" text; previously this hardcoded Modrinth's actual wordmark+icon as
  raw SVG path data)
- `packages/ui/src/components/brand/TextLogo.vue` (titlebar wordmark, used by both the desktop app
  and the web frontend header/footer — inlines the mark next to "Dyad" text; previously a generic
  "PROJECT" placeholder with an unrelated ring/checkmark icon)

## Discord Rich Presence art asset

Discord's Rich Presence image is uploaded by hand in the Discord Developer Portal (Rich Presence →
Art Assets on the Dyad application), not read from this repo. The asset key the code refers to is
`logo_square_1024` (see `packages/app-lib/src/state/discord.rs`). It is a **fully square,
1024×1024** export of the mark, with no rounded corners and no inset border (Discord rounds it
itself). To re-make it, render `mark.svg` with `rx="0"` on the tile and the inset border rect
removed, through the same `tauri icon` tool as below, and take the 1024px `ios/AppIcon-512@2x.png`
from its output.

## Regenerating the rest

Everything under `apps/app/icons/` that's a flat raster (`icon.png`, `icon.ico`, `favicon.ico`,
`128x128.png`, `128x128@2x.png`, the Windows `Square*Logo.png` tile set, `StoreLogo.png`, and a
fresh `icon.icns`) still holds the grey `PLACEHOLDER` box from the branding-compliance pass —
generating those correctly needs Tauri's own tool, not hand-authored files. Tauri's CLI accepts an
SVG source directly, so once dependencies are installed:

```bash
pnpm install
pnpm --filter @modrinth/app tauri icon docs/branding/mark.svg
```

That single command regenerates the entire platform icon set (Windows `.ico`, macOS `.icns`, and
every PNG size) from `mark.svg`. Re-run it any time `mark.svg` changes.
