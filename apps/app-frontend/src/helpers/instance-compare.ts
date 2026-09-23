import { invoke } from '@tauri-apps/api/core'

import type { ContentFileProjectType, InstanceLoader } from '@/helpers/types'

export type ComparisonState = 'identical' | 'only_in_some' | 'version_differs' | 'enabled_differs'

export type ComparedInstance = {
	instance_id: string
	name: string
	game_version: string
	loader: InstanceLoader
	loader_version?: string | null
}

export type ContentComparisonEntry = {
	file_name: string
	version_number?: string | null
	enabled: boolean
}

export type ContentComparisonRow = {
	project_type: ContentFileProjectType
	display_name: string
	entries: (ContentComparisonEntry | null)[]
	/**
	 * Every state that applies to this row. A row can be both `only_in_some` and
	 * `version_differs`/`enabled_differs` at once (e.g. present in two of three
	 * instances, with those two on different versions). `identical` only ever
	 * appears alone.
	 */
	states: ComparisonState[]
}

export type ComparisonReport = {
	instances: ComparedInstance[]
	content: ContentComparisonRow[]
}

export type ComparisonExportFormat = 'markdown' | 'json'

export async function compare_instances(instanceIds: string[]): Promise<ComparisonReport> {
	return await invoke('plugin:instance|instance_compare', { instanceIds })
}

export async function export_comparison(
	instanceIds: string[],
	format: ComparisonExportFormat,
): Promise<string> {
	return await invoke('plugin:instance|instance_compare_export', { instanceIds, format })
}
