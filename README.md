# Dyad Launcher

![Issues](https://img.shields.io/github/issues-raw/creulcat/modrinthcode?color=c78aff&label=issues&style=for-the-badge)
![Pull Requests](https://img.shields.io/github/issues-pr-raw/creulcat/modrinthcode?color=c78aff&label=PRs&style=for-the-badge)
![Contributors](https://img.shields.io/github/contributors/creulcat/modrinthcode?color=c78aff&label=contributors&style=for-the-badge)
![Lines of Code](https://img.shields.io/endpoint?url=https://loctopus.creeperkatze.dev/github/creulcat/modrinthcode/badge?style=flat&logoColor=white&color=c78aff&style=for-the-badge)
![Commit Activity](https://img.shields.io/github/commit-activity/m/creulcat/modrinthcode?color=c78aff&label=commits&style=for-the-badge)
![Last Commit](https://img.shields.io/github/last-commit/creulcat/modrinthcode?color=c78aff&label=last%20commit&style=for-the-badge)

## Dyad Launcher

Dyad Launcher is a fork of the [Modrinth Monorepo](https://github.com/modrinth/code), focused on
the **desktop app**. The name comes from *dyad* (Greek, "a pair") — the core feature being able to
run two linked instances of the same setup side by side. See [COPYING.md](COPYING.md) for what
changes forks of this repository need to make, and [docs/GOALS.md](docs/GOALS.md) for what this
fork is specifically trying to build and why.

If you're not a developer and you've stumbled upon this repository, you can access the original
web interface on the [Modrinth website](https://modrinth.com) and download the latest release of
the official app [here](https://modrinth.com/app).

## Goals

This fork's focus is entirely the desktop launcher (`apps/app`, `apps/app-frontend`, and the
`theseus` library in `packages/app-lib`), primarily on Windows. In short:

1. **Concurrent multi-account launches** — open the same instance more than once at a time, each
   under a different Microsoft account, when the instance opts into it.
2. **Symlink-based resource sharing** — share mods, resource/shader packs, config/settings, and
   worlds between instances via configurable symlinks.
3. **Debloating** — strip telemetry/analytics, account/login promos & ads, and news/social panels
   from the desktop app, while keeping (and tuning) Discord Rich Presence.

Full detail and known tradeoffs are in [docs/GOALS.md](docs/GOALS.md).

## Development

This repository contains two primary packages. For detailed development information, please refer to their respective guides:

- [Website frontend](https://docs.modrinth.com/contributing/knossos/)
- [Desktop app](https://docs.modrinth.com/contributing/theseus/)

## Contributing

We welcome contributions! Before submitting any contributions, please read our [contributing guidelines](https://docs.modrinth.com/contributing/getting-started/).

If you plan to fork this repository for your own purposes, please review our [copying guidelines](COPYING.md).

## Security

If you discover a security vulnerability within our codebase, please follow our [responsible disclosure guidelines](https://modrinth.com/legal/security).

## Support

If you need help with the Modrinth web interface or app, please visit our [support page](https://support.modrinth.com). For general inquiries, you can also join our [Discord server](https://discord.modrinth.com).

## License

All packages in this repository are licensed under their respective licenses. Refer to the LICENSE file in each package for more information.
