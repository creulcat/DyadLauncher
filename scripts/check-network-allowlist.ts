import chalk from 'chalk'
import * as fs from 'fs'
import * as path from 'path'
import { fileURLToPath } from 'url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const ROOT = path.resolve(__dirname, '..')
const NETWORK_DOC = path.join(ROOT, 'docs/NETWORK.md')

// Goal 7, item 7: fail CI when a hardcoded hostname shows up in the desktop app's source, config or
// CSP that isn't documented in docs/NETWORK.md. This only catches hostnames written literally in
// these files -- it can't catch a URL built at runtime from an API response. See docs/NETWORK.md's
// intro for the full caveat.

// Files/globs that make up "the desktop app" for this check. Deliberately excludes the website
// (apps/frontend), labrinth, and other workspace packages -- those aren't Dyad-specific and have
// their own, much larger set of hosts that this check was never meant to police.
const SCAN_TARGETS: string[] = [
	'packages/app-lib/src',
	'packages/app-lib/.env',
	'packages/app-lib/.env.local',
	'packages/app-lib/.env.prod',
	'packages/app-lib/.env.staging',
	'apps/app/src',
	'apps/app/build.rs',
	'apps/app/tauri.conf.json',
	'apps/app/tauri-release.conf.json',
	'apps/app/tauri.macos.conf.json',
	'apps/app/tauri.linux.conf.json',
	'apps/app/capabilities',
	'apps/app-frontend/src',
	'apps/app-frontend/vite.config.ts',
	'apps/app-frontend/index.html',
]

const SCANNED_EXTENSIONS = new Set([
	'.rs',
	'.ts',
	'.vue',
	'.js',
	'.mjs',
	'.cjs',
	'.json',
	'.html',
	'.scss',
])

const EXCLUDED_DIRS = new Set(['node_modules', 'dist', '.turbo', '.sqlx', 'target'])

const URL_PATTERN = /\b(?:https?|wss?):\/\/([a-zA-Z0-9*][a-zA-Z0-9.*-]*)/g
const HOSTNAME_TOKEN_PATTERN = /^(\*\.)?[a-zA-Z0-9-]+(\.[a-zA-Z0-9-]+)+$/

const theme = {
	error: chalk.red,
	success: chalk.green,
	muted: chalk.gray,
	highlight: chalk.white.bold,
	file: chalk.cyan,
}

interface Occurrence {
	host: string
	file: string
	line: number
}

function isPathExcluded(p: string): boolean {
	return p.split(path.sep).some((segment) => EXCLUDED_DIRS.has(segment))
}

function findFiles(target: string): string[] {
	const abs = path.join(ROOT, target)
	if (!fs.existsSync(abs)) return []

	const stat = fs.statSync(abs)
	if (stat.isFile()) return [abs]

	const files: string[] = []
	function walk(dir: string) {
		for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
			const fullPath = path.join(dir, entry.name)
			if (isPathExcluded(fullPath)) continue

			if (entry.isDirectory()) {
				walk(fullPath)
			} else if (entry.isFile() && SCANNED_EXTENSIONS.has(path.extname(entry.name))) {
				files.push(fullPath)
			}
		}
	}
	walk(abs)
	return files
}

function extractHostname(rawHost: string): string {
	// Strip a trailing port, if any (host:port -- but not the "::" in wss:// itself, already consumed).
	return rawHost.split(':')[0]
}

function findHostOccurrences(filePath: string): Occurrence[] {
	const content = fs.readFileSync(filePath, 'utf-8')
	const relativePath = path.relative(ROOT, filePath)
	const occurrences: Occurrence[] = []

	for (const match of content.matchAll(URL_PATTERN)) {
		const host = extractHostname(match[1])
		if (!HOSTNAME_TOKEN_PATTERN.test(host)) continue

		const line = content.slice(0, match.index).split('\n').length
		occurrences.push({ host, file: relativePath, line })
	}

	return occurrences
}

/** Every backtick-quoted, hostname-shaped token anywhere in docs/NETWORK.md is an allowed host. */
function loadAllowlist(): Set<string> {
	const content = fs.readFileSync(NETWORK_DOC, 'utf-8')
	const allowed = new Set<string>()

	for (const match of content.matchAll(/`([^`]+)`/g)) {
		const token = match[1]
		if (HOSTNAME_TOKEN_PATTERN.test(token)) {
			allowed.add(token)
		}
	}

	return allowed
}

function isAllowed(host: string, allowlist: Set<string>): boolean {
	if (allowlist.has(host)) return true
	if (host === 'localhost' || host === '127.0.0.1') return allowlist.has(host)

	// A `*.example.com` allowlist entry covers `sub.example.com`, `a.b.example.com`, etc.
	for (const entry of allowlist) {
		if (entry.startsWith('*.') && (host === entry.slice(2) || host.endsWith(entry.slice(1)))) {
			return true
		}
	}

	return false
}

function main() {
	const allowlist = loadAllowlist()

	if (allowlist.size === 0) {
		console.error(theme.error(`No hostnames found in ${path.relative(ROOT, NETWORK_DOC)}.`))
		process.exit(1)
	}

	const files = SCAN_TARGETS.flatMap(findFiles)
	const violations: Occurrence[] = []

	for (const file of files) {
		for (const occurrence of findHostOccurrences(file)) {
			if (!isAllowed(occurrence.host, allowlist)) {
				violations.push(occurrence)
			}
		}
	}

	console.log()
	console.log(
		theme.muted(
			`  Scanned ${files.length} file(s) against ${allowlist.size} allowed host(s) from docs/NETWORK.md`,
		),
	)
	console.log()

	if (violations.length === 0) {
		console.log(theme.success('  No undocumented hosts found.'))
		console.log()
		process.exit(0)
	}

	console.log(theme.error(`  Found ${violations.length} undocumented host(s):`))
	console.log()
	for (const violation of violations) {
		console.log(
			`    ${theme.file(`${violation.file}:${violation.line}`)} contacts ${theme.highlight(violation.host)}`,
		)
	}
	console.log()
	console.log(
		theme.muted(
			"  Add each new host to docs/NETWORK.md (with why it's contacted and whether it's opt-in),\n" +
				"  or fix the code if it shouldn't be contacting it at all.",
		),
	)
	console.log()

	process.exit(1)
}

main()
