/**
 * Discord Rich Presence: tells the backend which part of the launcher the user is in, which is
 * what the presence shows while no visible instance is running.
 * Only a coarse category is ever sent, never a project or instance name.
 */
import { invoke } from '@tauri-apps/api/core'

export type LauncherActivity =
	| 'in_launcher'
	| 'browsing_mods'
	| 'looking_at_instances'
	| 'changing_skins'
	| 'looking_at_screenshots'

// The library is part of the home page, so home counts as looking at instances.
// An instance's own screenshots tab stays with the instance; only the global page counts here
export function launcherActivityForPath(path: string): LauncherActivity {
	if (path === '/' || path.startsWith('/instance/')) return 'looking_at_instances'
	if (path.startsWith('/browse/') || path.startsWith('/project/') || path.startsWith('/user/')) {
		return 'browsing_mods'
	}
	if (path === '/skins') return 'changing_skins'
	if (path === '/screenshots') return 'looking_at_screenshots'
	return 'in_launcher'
}

export async function set_launcher_activity(activity: LauncherActivity): Promise<void> {
	return await invoke('plugin:discord|discord_set_launcher_activity', { activity })
}

// Discord rate-limits presence updates, so wait for navigation to settle before reporting
const REPORT_DELAY_MS = 1000
let reportTimer: ReturnType<typeof setTimeout> | undefined

// Failures are ignored on purpose: Rich Presence is never worth interrupting navigation for
export function report_launcher_page(path: string): void {
	clearTimeout(reportTimer)
	reportTimer = setTimeout(() => {
		set_launcher_activity(launcherActivityForPath(path)).catch(() => {})
	}, REPORT_DELAY_MS)
}
