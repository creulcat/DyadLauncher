<script setup lang="ts">
import {
	ArrowLeftRightIcon,
	BoxIcon,
	BracesIcon,
	ClipboardCopyIcon,
	GlassesIcon,
	PaintbrushIcon,
	PlusIcon,
	SaveIcon,
	SearchIcon,
	XIcon,
} from '@modrinth/assets'
import {
	Button,
	Combobox,
	type ComboboxOption,
	commonMessages,
	defineMessages,
	EmptyState,
	type FilterPillOption,
	FilterPills,
	formatLoader,
	IconButton,
	injectNotificationManager,
	Input,
	ReadyTransition,
	SimpleBadge,
	Table,
	type TableColumn,
	Tabs,
	type TabsTab,
	useVIntl,
} from '@modrinth/ui'
import { save } from '@tauri-apps/plugin-dialog'
import { writeTextFile } from '@tauri-apps/plugin-fs'
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { toError } from '@/helpers/errors'
import { list as listInstances } from '@/helpers/instance'
import {
	compare_instances,
	type ComparisonReport,
	type ComparisonState,
	type ContentComparisonEntry,
	export_comparison,
} from '@/helpers/instance-compare'
import type { ContentFileProjectType, GameInstance } from '@/helpers/types'
import { copyToClipboard } from '@/helpers/utils'

const route = useRoute()
const router = useRouter()
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()

const messages = defineMessages({
	title: { id: 'app.compare.title', defaultMessage: 'Compare instances' },
	gameVersion: { id: 'app.compare.game-version', defaultMessage: 'Minecraft version' },
	loader: { id: 'app.compare.loader', defaultMessage: 'Loader' },
	searchPlaceholder: {
		id: 'app.compare.search-placeholder',
		defaultMessage: 'Search content...',
	},
	filterEnabledSomewhere: {
		id: 'app.compare.filter.enabled-somewhere',
		defaultMessage: 'Enabled somewhere',
	},
	filterDisabledSomewhere: {
		id: 'app.compare.filter.disabled-somewhere',
		defaultMessage: 'Disabled somewhere',
	},
	copy: { id: 'app.compare.copy', defaultMessage: 'Copy' },
	noContent: {
		id: 'app.compare.no-content',
		defaultMessage: 'No content matches the current filters',
	},
	invalidSelection: {
		id: 'app.compare.invalid-selection',
		defaultMessage: 'Select at least two instances to compare',
	},
	content: { id: 'app.compare.content', defaultMessage: 'Content' },
	status: { id: 'app.compare.status', defaultMessage: 'Status' },
	stateIdentical: { id: 'app.compare.state.identical', defaultMessage: 'Identical' },
	stateOnlyInSome: { id: 'app.compare.state.only-in-some', defaultMessage: 'Only in some' },
	stateVersionDiffers: {
		id: 'app.compare.state.version-differs',
		defaultMessage: 'Version differs',
	},
	stateEnabledDiffers: {
		id: 'app.compare.state.enabled-differs',
		defaultMessage: 'Enabled state differs',
	},
	tabAll: { id: 'app.compare.tab.all', defaultMessage: 'All' },
	tabMods: { id: 'app.compare.tab.mods', defaultMessage: 'Mods' },
	tabResourcePacks: { id: 'app.compare.tab.resource-packs', defaultMessage: 'Resource Packs' },
	tabShaderPacks: { id: 'app.compare.tab.shader-packs', defaultMessage: 'Shader Packs' },
	tabDataPacks: { id: 'app.compare.tab.data-packs', defaultMessage: 'Data Packs' },
	formatMarkdown: { id: 'app.compare.format.markdown', defaultMessage: 'Markdown' },
	formatJson: { id: 'app.compare.format.json', defaultMessage: 'JSON' },
	summary: {
		id: 'app.compare.summary',
		defaultMessage:
			'{identical} identical · {onlyInSome} only in some · {versionDiffers} version differs · {enabledDiffers} enabled differs',
	},
	summaryOverlapNote: {
		id: 'app.compare.summary-overlap-note',
		defaultMessage: '(a row can count toward more than one status)',
	},
	addInstance: { id: 'app.compare.add-instance', defaultMessage: 'Add instance' },
	noMoreInstances: {
		id: 'app.compare.no-more-instances',
		defaultMessage: 'No other instances available',
	},
	removeInstance: { id: 'app.compare.remove-instance', defaultMessage: 'Remove from comparison' },
	removeInstanceMinimum: {
		id: 'app.compare.remove-instance-minimum',
		defaultMessage: 'At least two instances are required to compare',
	},
})

const instanceIds = computed(() => {
	const raw = route.query.ids
	const value = Array.isArray(raw) ? raw[0] : raw
	return (value ?? '')
		.split(',')
		.map((id) => id.trim())
		.filter(Boolean)
})

const report = ref<ComparisonReport | null>(null)
const loading = ref(true)
const loadError = ref<string | null>(null)
const exporting = ref(false)
const allInstances = ref<GameInstance[]>([])
const addInstanceSelection = ref<string | undefined>(undefined)

const activeProjectType = ref<'all' | ContentFileProjectType>('all')
const stateFilter = ref<string[]>([])
const enabledFilter = ref<string[]>([])
const searchQuery = ref('')
const exportFormat = ref<'markdown' | 'json'>('markdown')

const projectTypeIcons: Record<ContentFileProjectType, unknown> = {
	mod: BoxIcon,
	datapack: BracesIcon,
	resourcepack: PaintbrushIcon,
	shaderpack: GlassesIcon,
}

const projectTypeTabs = computed<TabsTab[]>(() => [
	{ value: 'all', label: formatMessage(messages.tabAll) },
	{ value: 'mod', label: formatMessage(messages.tabMods), icon: projectTypeIcons.mod },
	{
		value: 'resourcepack',
		label: formatMessage(messages.tabResourcePacks),
		icon: projectTypeIcons.resourcepack,
	},
	{
		value: 'shaderpack',
		label: formatMessage(messages.tabShaderPacks),
		icon: projectTypeIcons.shaderpack,
	},
	{
		value: 'datapack',
		label: formatMessage(messages.tabDataPacks),
		icon: projectTypeIcons.datapack,
	},
])

const exportFormatTabs = computed<TabsTab[]>(() => [
	{ value: 'markdown', label: formatMessage(messages.formatMarkdown) },
	{ value: 'json', label: formatMessage(messages.formatJson) },
])

const stateFilterOptions = computed<FilterPillOption[]>(() => [
	{ id: 'identical', label: formatMessage(messages.stateIdentical) },
	{ id: 'only_in_some', label: formatMessage(messages.stateOnlyInSome) },
	{ id: 'version_differs', label: formatMessage(messages.stateVersionDiffers) },
	{ id: 'enabled_differs', label: formatMessage(messages.stateEnabledDiffers) },
])

const enabledFilterOptions = computed<FilterPillOption[]>(() => [
	{ id: 'enabled', label: formatMessage(messages.filterEnabledSomewhere) },
	{ id: 'disabled', label: formatMessage(messages.filterDisabledSomewhere) },
])

async function loadReport() {
	if (instanceIds.value.length < 2) {
		report.value = null
		loadError.value = formatMessage(messages.invalidSelection)
		loading.value = false
		return
	}

	loading.value = true
	loadError.value = null
	try {
		report.value = await compare_instances(instanceIds.value)
	} catch (error) {
		loadError.value = toError(error).message
	} finally {
		loading.value = false
	}
}

onMounted(() => {
	loadReport()
	listInstances()
		.then((instances) => {
			allInstances.value = instances
		})
		.catch(handleError)
})
watch(instanceIds, loadReport)

const addableInstanceOptions = computed<ComboboxOption<string>[]>(() =>
	allInstances.value
		.filter((instance) => !instanceIds.value.includes(instance.id))
		.map((instance) => ({ value: instance.id, label: instance.name })),
)

watch(addInstanceSelection, (selectedInstanceId) => {
	if (!selectedInstanceId) return

	router.replace({
		path: '/compare',
		query: { ids: [...instanceIds.value, selectedInstanceId].join(',') },
	})
	addInstanceSelection.value = undefined
})

function removeInstance(instanceId: string) {
	if (instanceIds.value.length <= 2) return

	router.replace({
		path: '/compare',
		query: { ids: instanceIds.value.filter((id) => id !== instanceId).join(',') },
	})
}

const typeFilteredRows = computed(() => {
	if (!report.value) return []
	if (activeProjectType.value === 'all') return report.value.content
	return report.value.content.filter((row) => row.project_type === activeProjectType.value)
})

const visibleRows = computed(() => {
	let rows = typeFilteredRows.value

	if (stateFilter.value.length > 0) {
		rows = rows.filter((row) => row.states.some((state) => stateFilter.value.includes(state)))
	}

	if (enabledFilter.value.length > 0) {
		rows = rows.filter((row) => {
			const presentEntries = row.entries.filter(
				(entry): entry is ContentComparisonEntry => entry !== null,
			)
			return (
				(enabledFilter.value.includes('enabled') &&
					presentEntries.some((entry) => entry.enabled)) ||
				(enabledFilter.value.includes('disabled') && presentEntries.some((entry) => !entry.enabled))
			)
		})
	}

	const query = searchQuery.value.trim().toLowerCase()
	if (query) {
		rows = rows.filter((row) => row.display_name.toLowerCase().includes(query))
	}

	return rows
})

const summaryCounts = computed(() => {
	const counts = { identical: 0, onlyInSome: 0, versionDiffers: 0, enabledDiffers: 0 }
	for (const row of typeFilteredRows.value) {
		for (const state of row.states) {
			if (state === 'identical') counts.identical++
			else if (state === 'only_in_some') counts.onlyInSome++
			else if (state === 'version_differs') counts.versionDiffers++
			else if (state === 'enabled_differs') counts.enabledDiffers++
		}
	}
	return counts
})

const hasOverlappingStates = computed(() =>
	typeFilteredRows.value.some((row) => row.states.length > 1),
)

const tableColumns = computed<TableColumn[]>(() => {
	if (!report.value) return []
	return [
		{ key: 'content', label: formatMessage(messages.content) },
		...report.value.instances.map((instance) => ({
			key: `instance_${instance.instance_id}`,
			label: instance.name,
		})),
		{ key: 'status', label: formatMessage(messages.status), width: '14rem' },
	]
})

const tableRows = computed(() => {
	if (!report.value) return []
	const instances = report.value.instances
	return visibleRows.value.map((row, index) => {
		const record: Record<string, unknown> = {
			key: `${row.project_type}-${index}-${row.display_name}`,
			content: row.display_name,
			states: row.states,
		}
		instances.forEach((instance, instanceIndex) => {
			record[`instance_${instance.instance_id}`] = formatEntry(row.entries[instanceIndex])
		})
		return record
	})
})

function formatEntry(entry: ContentComparisonEntry | null): string {
	if (!entry) return '—'
	const label = entry.version_number ?? entry.file_name
	return entry.enabled ? label : `${label} (disabled)`
}

function stateLabel(state: ComparisonState): string {
	switch (state) {
		case 'identical':
			return formatMessage(messages.stateIdentical)
		case 'only_in_some':
			return formatMessage(messages.stateOnlyInSome)
		case 'version_differs':
			return formatMessage(messages.stateVersionDiffers)
		case 'enabled_differs':
			return formatMessage(messages.stateEnabledDiffers)
		default:
			return state
	}
}

function stateColor(state: ComparisonState): 'green' | 'blue' | 'orange' | 'purple' {
	switch (state) {
		case 'identical':
			return 'green'
		case 'only_in_some':
			return 'blue'
		case 'version_differs':
			return 'orange'
		case 'enabled_differs':
			return 'purple'
		default:
			return 'blue'
	}
}

async function copyReport() {
	if (instanceIds.value.length < 2 || exporting.value) return

	exporting.value = true
	try {
		const text = await export_comparison(instanceIds.value, exportFormat.value)
		await copyToClipboard(text)
	} catch (error) {
		handleError(toError(error))
	} finally {
		exporting.value = false
	}
}

async function saveReport() {
	if (instanceIds.value.length < 2 || exporting.value) return

	const extension = exportFormat.value === 'markdown' ? 'md' : 'json'
	const outputPath = await save({
		defaultPath: `instance-comparison.${extension}`,
		filters: [
			{
				name: exportFormat.value === 'markdown' ? 'Markdown' : 'JSON',
				extensions: [extension],
			},
		],
	})
	if (!outputPath) return

	exporting.value = true
	try {
		const text = await export_comparison(instanceIds.value, exportFormat.value)
		await writeTextFile(outputPath, text)
	} catch (error) {
		handleError(toError(error))
	} finally {
		exporting.value = false
	}
}
</script>

<template>
	<div class="compare-layout box-border grow p-4 flex flex-col gap-4">
		<div class="flex flex-wrap items-center justify-between gap-3">
			<h1 class="m-0 flex items-center gap-2 text-2xl font-bold">
				<ArrowLeftRightIcon class="size-6 shrink-0" />
				{{ formatMessage(messages.title) }}
			</h1>
			<Combobox
				v-model="addInstanceSelection"
				searchable
				clearable
				class="w-64"
				:options="addableInstanceOptions"
				:placeholder="formatMessage(messages.addInstance)"
				:no-options-message="formatMessage(messages.noMoreInstances)"
			>
				<template #prefix>
					<PlusIcon class="size-5 shrink-0 text-primary" />
				</template>
			</Combobox>
		</div>

		<ReadyTransition :pending="loading">
			<EmptyState
				v-if="loadError"
				type="error"
				:heading="formatMessage(commonMessages.errorLabel)"
				:description="loadError"
			/>
			<div v-else-if="report" class="flex flex-col gap-4">
				<div class="overflow-hidden rounded-2xl border border-solid border-surface-4">
					<div
						class="grid"
						:style="{
							gridTemplateColumns: `12rem repeat(${report.instances.length}, minmax(10rem, 1fr))`,
						}"
					>
						<div class="bg-surface-3 p-3"></div>
						<div
							v-for="instance in report.instances"
							:key="`${instance.instance_id}-header`"
							class="flex min-w-0 items-center justify-between gap-2 bg-surface-3 p-3 font-semibold text-contrast"
						>
							<span class="truncate">{{ instance.name }}</span>
							<IconButton
								v-tooltip="
									report.instances.length <= 2
										? formatMessage(messages.removeInstanceMinimum)
										: formatMessage(messages.removeInstance)
								"
								:label="formatMessage(messages.removeInstance)"
								:disabled="report.instances.length <= 2"
								size="sm"
								class="shrink-0"
								@click="removeInstance(instance.instance_id)"
							>
								<XIcon />
							</IconButton>
						</div>
						<div class="bg-surface-2 p-3 text-secondary">
							{{ formatMessage(messages.gameVersion) }}
						</div>
						<div
							v-for="instance in report.instances"
							:key="`${instance.instance_id}-gv`"
							class="bg-surface-1.5 p-3"
						>
							{{ instance.game_version }}
						</div>
						<div class="bg-surface-2 p-3 text-secondary">
							{{ formatMessage(messages.loader) }}
						</div>
						<div
							v-for="instance in report.instances"
							:key="`${instance.instance_id}-loader`"
							class="bg-surface-1.5 p-3"
						>
							{{ formatLoader(formatMessage, instance.loader)
							}}{{ instance.loader_version ? ` ${instance.loader_version}` : '' }}
						</div>
					</div>
				</div>

				<div class="flex flex-wrap items-center justify-between gap-3">
					<div class="flex flex-wrap items-center gap-3">
						<Tabs v-model:value="activeProjectType" :tabs="projectTypeTabs" />
						<Input
							v-model="searchQuery"
							type="text"
							:icon="SearchIcon"
							:placeholder="formatMessage(messages.searchPlaceholder)"
							clearable
						/>
					</div>
					<div class="flex flex-wrap items-center gap-2">
						<Tabs v-model:value="exportFormat" :tabs="exportFormatTabs" />
						<Button type="outlined" :disabled="exporting" @click="copyReport">
							<ClipboardCopyIcon />
							{{ formatMessage(messages.copy) }}
						</Button>
						<Button type="outlined" :disabled="exporting" @click="saveReport">
							<SaveIcon />
							{{ formatMessage(commonMessages.saveButton) }}
						</Button>
					</div>
				</div>

				<div class="flex flex-wrap items-center gap-x-6 gap-y-2">
					<FilterPills v-model="stateFilter" :options="stateFilterOptions">
						<template #all>{{ formatMessage(commonMessages.allProjectType) }}</template>
					</FilterPills>
					<FilterPills v-model="enabledFilter" :options="enabledFilterOptions">
						<template #all>{{ formatMessage(commonMessages.allProjectType) }}</template>
					</FilterPills>
				</div>

				<p class="m-0 text-sm text-secondary">
					{{
						formatMessage(messages.summary, {
							identical: summaryCounts.identical,
							onlyInSome: summaryCounts.onlyInSome,
							versionDiffers: summaryCounts.versionDiffers,
							enabledDiffers: summaryCounts.enabledDiffers,
						})
					}}
					<template v-if="hasOverlappingStates">
						{{ formatMessage(messages.summaryOverlapNote) }}
					</template>
				</p>

				<Table :columns="tableColumns" :data="tableRows" row-key="key">
					<template #cell-status="{ row }">
						<div class="flex flex-wrap items-center gap-1.5">
							<SimpleBadge
								v-for="state in row.states as ComparisonState[]"
								:key="state"
								:color="stateColor(state)"
								:formatted-name="stateLabel(state)"
							/>
						</div>
					</template>
					<template #empty-state>
						<EmptyState :heading="formatMessage(messages.noContent)" />
					</template>
				</Table>
			</div>
		</ReadyTransition>
	</div>
</template>
