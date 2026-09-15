/**
 * Goal 6 (see docs/goal-6-import-design.md): frontend wrappers for importing
 * instances, content, and settings from the official Modrinth App.
 *
 * These types are hand-written to match the `camelCase`-serialized shape of
 * the Rust structs in `packages/app-lib/src/api/migrate_modrinth_app/{mod,execute}.rs`
 * - command parameter/return types aren't ts-rs-generated in this codebase
 * (only backend->frontend events are, see `@/generated/app-events`), so every
 * other command wrapper here (e.g. `PackExportCandidate` in `./instance.ts`)
 * follows the same hand-written-type pattern.
 */
import { invoke } from '@tauri-apps/api/core'

import type { InstallJobSnapshot } from './install'
import type { InstanceLoader } from './types'

export type ContentCategory =
	| 'mods'
	| 'resource_packs'
	| 'shader_packs'
	| 'config'
	| 'saves'
	| 'screenshots'
	| 'logs'

export interface DetectedSource {
	settingsDir: string
	dbPath: string
}

export interface ImportContentCategory {
	category: ContentCategory
	fileCount: number
	/** `null` for the `saves` category - see the Rust doc comment on `ImportContentCategory::total_size`. */
	totalSize: number | null
	defaultSelected: boolean
}

export interface ImportWorldCandidate {
	folderName: string
	/** Unix timestamp (seconds), if readable. */
	modified: number | null
}

export type SymlinkAction = 'copy' | 'recreate' | 'ignore'

/**
 * A whole category folder (e.g. `mods`) or a whole top-level world folder
 * (`saves/<world>`) that is itself a symlink or Windows junction. Only these
 * two shapes are detected - a symlink nested deeper inside an otherwise-real
 * folder, or one pointing at a single file, is left to be copied as real
 * content like today.
 */
export interface ImportSymlinkCandidate {
	/** Matches a category folder name (e.g. `"mods"`) or `"saves/<world>"`. */
	relativePath: string
	category: ContentCategory
	/** Absolute path this link ultimately resolves to. */
	target: string
	/**
	 * `false` means `target` lies inside the official Modrinth App's own
	 * managed directories - recreating the link would leave the imported
	 * instance depending on the source install staying in place, so a
	 * "recreate" choice for this one should be sent as `"copy"` instead.
	 */
	targetOutsideSourceApp: boolean
}

export interface ImportInstanceCandidate {
	sourceId: string
	instanceDir: string
	name: string
	iconPath: string | null
	loader: InstanceLoader
	rawLoader: string | null
	loaderVersion: string | null
	gameVersion: string | null
	created: number
	modified: number
	lastPlayed: number | null
	/** Total seconds played on the source instance. */
	totalTimePlayed: number
	/**
	 * Only categories that actually have files are included. A category
	 * that's itself a symlink/junction is never listed here - see `symlinks`.
	 */
	categories: ImportContentCategory[]
	/**
	 * A world that's itself a symlink/junction is still listed here (for its
	 * name/modified date) as well as in `symlinks`.
	 */
	worlds: ImportWorldCandidate[]
	symlinks: ImportSymlinkCandidate[]
}

export interface ImportSettingsCandidate {
	extraLaunchArgs: string[] | null
	customEnvVars: [string, string][] | null
	memoryMaximumMb: number | null
	forceFullscreen: boolean | null
	gameResolution: [number, number] | null
	hookPreLaunch: string | null
	hookWrapper: string | null
	hookPostExit: string | null
}

export interface ImportJavaVersionCandidate {
	majorVersion: number
	fullVersion: string
	architecture: string
	path: string
}

export interface ImportPreview {
	source: DetectedSource
	instances: ImportInstanceCandidate[]
	settings: ImportSettingsCandidate
	javaVersions: ImportJavaVersionCandidate[]
	/** Non-fatal notes about optional source columns this reader skipped. */
	compatibilityNotes: string[]
}

export interface ImportSelection {
	categories: ContentCategory[]
	worlds: string[]
	/**
	 * Per-`ImportSymlinkCandidate.relativePath` action for any detected
	 * symlink/junction included via `categories`/`worlds` above. Send
	 * `"recreate"` only when the matching candidate's `targetOutsideSourceApp`
	 * is `true` - otherwise send `"copy"`, since the backend trusts this
	 * classification rather than re-deriving it itself.
	 */
	symlinkActions: Record<string, SymlinkAction>
}

export interface SettingsImportSelection {
	extraLaunchArgs: boolean
	customEnvVars: boolean
	memoryMaximum: boolean
	forceFullscreen: boolean
	gameResolution: boolean
	/** Covers pre-launch/wrapper/post-exit hooks together. */
	hooks: boolean
	javaVersions: boolean
}

export interface InstallImportModrinthAppRequest {
	sourceInstanceDir: string
	name: string
	gameVersion: string
	loader: InstanceLoader
	loaderVersion: string | null
	iconPath: string | null
	selection: ImportSelection
	deleteSourceAfterImport: boolean
	lastPlayed: number | null
	totalTimePlayed: number
}

/** Looks for an official Modrinth App install at `sourceDir`, or the platform default if omitted. */
export async function detectModrinthAppInstall(
	sourceDir?: string | null,
): Promise<DetectedSource | null> {
	return await invoke('plugin:migrate-modrinth-app|detect_modrinth_app_install', {
		sourceDir: sourceDir ?? null,
	})
}

/** Whether the official Modrinth App appears to currently be running (it must be closed first). */
export async function isModrinthAppRunning(): Promise<boolean> {
	return await invoke('plugin:migrate-modrinth-app|is_modrinth_app_running')
}

/** Builds a full read-only preview of what could be imported from `source`. */
export async function previewModrinthAppImport(source: DetectedSource): Promise<ImportPreview> {
	return await invoke('plugin:migrate-modrinth-app|preview_modrinth_app_import', { source })
}

/** Applies the selected fields from a settings/Java-path preview onto Dyad's own settings. */
export async function applyModrinthAppSettings(
	candidate: ImportSettingsCandidate,
	javaVersions: ImportJavaVersionCandidate[],
	selection: SettingsImportSelection,
): Promise<void> {
	return await invoke('plugin:migrate-modrinth-app|apply_modrinth_app_settings', {
		candidate,
		javaVersions,
		selection,
	})
}

/** Starts an install job that creates one instance and copies the selected content into it. */
export async function installImportModrinthAppInstance(
	request: InstallImportModrinthAppRequest,
): Promise<InstallJobSnapshot> {
	return await invoke('plugin:install|install_import_modrinth_app_instance', { request })
}
