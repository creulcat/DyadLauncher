import { getVersion } from '@tauri-apps/api/app'
import { fetch } from '@tauri-apps/plugin-http'
import { arch as getArch, platform as getPlatform } from '@tauri-apps/plugin-os'
import { computed, ref } from 'vue'

const UPDATES_MANIFEST_URL =
	'https://github.com/creulcat/DyadLauncher/releases/latest/download/updates.json'
export const RELEASES_PAGE_URL = 'https://github.com/creulcat/DyadLauncher/releases/latest'
const CHECK_INTERVAL_MS = 60 * 60 * 1000

interface UpdateManifestPlatform {
	install_urls: string[]
}

interface UpdateManifest {
	version: string
	notes: string
	pub_date: string
	platforms: Record<string, UpdateManifestPlatform>
}

const currentVersion = ref<string | null>(null)
const latestVersion = ref<string | null>(null)
const releaseNotes = ref<string | null>(null)
const downloadUrl = ref<string | null>(null)
const checking = ref(false)
const lastCheckedAt = ref<number | null>(null)
const checkError = ref<string | null>(null)

let intervalHandle: ReturnType<typeof setInterval> | null = null

function isNewerVersion(latest: string, current: string): boolean {
	const parse = (v: string) => v.split('.').map((n) => Number.parseInt(n, 10) || 0)
	const latestParts = parse(latest)
	const currentParts = parse(current)
	for (let i = 0; i < Math.max(latestParts.length, currentParts.length); i++) {
		const l = latestParts[i] ?? 0
		const c = currentParts[i] ?? 0
		if (l !== c) return l > c
	}
	return false
}

async function resolvePlatformKey(): Promise<string | null> {
	const platform = await getPlatform()
	if (platform === 'macos') {
		const arch = await getArch()
		return arch === 'aarch64' ? 'darwin-aarch64' : 'darwin-x86_64'
	}
	if (platform === 'linux') return 'linux-x86_64'
	if (platform === 'windows') return 'windows-x86_64'
	return null
}

export const appUpdateCheck = {
	currentVersion,
	latestVersion,
	releaseNotes,
	downloadUrl,
	checking,
	lastCheckedAt,
	checkError,
	updateAvailable: computed(
		() =>
			!!latestVersion.value &&
			!!currentVersion.value &&
			isNewerVersion(latestVersion.value, currentVersion.value),
	),
}

export async function checkForAppUpdate(): Promise<void> {
	if (checking.value) return
	checking.value = true
	checkError.value = null
	try {
		if (!currentVersion.value) {
			currentVersion.value = await getVersion()
		}

		const response = await fetch(UPDATES_MANIFEST_URL, { method: 'GET' })
		if (!response.ok) {
			throw new Error(`Failed to fetch update manifest: ${response.status}`)
		}
		const manifest = (await response.json()) as UpdateManifest

		latestVersion.value = manifest.version
		releaseNotes.value = manifest.notes ?? null

		const platformKey = await resolvePlatformKey()
		const platformInfo = platformKey ? manifest.platforms[platformKey] : undefined
		downloadUrl.value = platformInfo?.install_urls?.[0] ?? null
		console.log('App update check resolved:', {
			currentVersion: currentVersion.value,
			latestVersion: latestVersion.value,
			platformKey,
			availablePlatformKeys: Object.keys(manifest.platforms),
			downloadUrl: downloadUrl.value,
		})
	} catch (error) {
		console.warn('Failed to check for app updates:', error)
		checkError.value = error instanceof Error ? error.message : String(error)
	} finally {
		checking.value = false
		lastCheckedAt.value = Date.now()
	}
}

export function startAppUpdateChecks(): void {
	if (intervalHandle !== null) return
	void checkForAppUpdate()
	intervalHandle = setInterval(() => void checkForAppUpdate(), CHECK_INTERVAL_MS)
}

export function stopAppUpdateChecks(): void {
	if (intervalHandle !== null) {
		clearInterval(intervalHandle)
		intervalHandle = null
	}
}
