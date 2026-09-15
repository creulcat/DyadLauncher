<script setup lang="ts">
import {
	ChevronRightIcon,
	CoffeeIcon,
	FolderSearchIcon,
	ImportIcon,
	LoaderCircleIcon,
	TriangleAlertIcon,
	XCircleIcon,
	XIcon,
} from '@modrinth/assets'
import {
	Button,
	Checkbox,
	Collapsible,
	commonMessages,
	defineMessages,
	IconButton,
	injectNotificationManager,
	NewModal,
	ProgressBar,
	useFormatBytes,
	useVIntl,
} from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, reactive, ref } from 'vue'

import {
	getErrorMessage,
	install_job_cancel,
	type InstallJobSnapshot,
	isInstallJobFinished,
} from '@/helpers/install'
import {
	applyModrinthAppSettings,
	type ContentCategory,
	type DetectedSource,
	detectModrinthAppInstall,
	type ImportInstanceCandidate,
	type ImportPreview,
	type ImportSelection,
	type ImportSymlinkCandidate,
	installImportModrinthAppInstance,
	isModrinthAppRunning,
	previewModrinthAppImport,
	type SettingsImportSelection,
	type SymlinkAction,
} from '@/helpers/migrate-modrinth-app'
import { injectAppEvents } from '@/providers/app-events'

import SymlinkActionSelector from './SymlinkActionSelector.vue'

type Step = 'detecting' | 'need-folder' | 'running' | 'error' | 'preview' | 'importing'

interface ActiveJob {
	instanceName: string
	snapshot: InstallJobSnapshot
}

const { handleError } = injectNotificationManager()
const appEvents = injectAppEvents()
const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()

const messages = defineMessages({
	header: {
		id: 'app.migrate-modrinth-app.header',
		defaultMessage: 'Import from Modrinth App',
	},
	detecting: {
		id: 'app.migrate-modrinth-app.detecting',
		defaultMessage: 'Looking for an official Modrinth App install...',
	},
	needFolderTitle: {
		id: 'app.migrate-modrinth-app.need-folder.title',
		defaultMessage: "Couldn't find a Modrinth App install",
	},
	needFolderText: {
		id: 'app.migrate-modrinth-app.need-folder.text',
		defaultMessage: 'Pick the Modrinth App data folder manually (the one containing app.db).',
	},
	browse: {
		id: 'app.migrate-modrinth-app.browse',
		defaultMessage: 'Browse...',
	},
	runningTitle: {
		id: 'app.migrate-modrinth-app.running.title',
		defaultMessage: 'Modrinth App is still running',
	},
	runningText: {
		id: 'app.migrate-modrinth-app.running.text',
		defaultMessage: 'Close the official Modrinth App completely, then try again.',
	},
	tryAgain: {
		id: 'app.migrate-modrinth-app.try-again',
		defaultMessage: 'Try again',
	},
	errorTitle: {
		id: 'app.migrate-modrinth-app.error.title',
		defaultMessage: "Couldn't read the Modrinth App install",
	},
	instancesTitle: {
		id: 'app.migrate-modrinth-app.instances-title',
		defaultMessage: 'Instances',
	},
	noGameVersion: {
		id: 'app.migrate-modrinth-app.no-game-version',
		defaultMessage: "Can't import - no recognizable game version",
	},
	worldsLabel: {
		id: 'app.migrate-modrinth-app.worlds-label',
		defaultMessage: 'Worlds',
	},
	launchOverrides: {
		id: 'app.migrate-modrinth-app.launch-overrides',
		defaultMessage:
			"Also import this instance's launch overrides (JVM args, memory, hooks, Java path)",
	},
	settingsTitle: {
		id: 'app.migrate-modrinth-app.settings-title',
		defaultMessage: 'Global settings',
	},
	settingsExtraLaunchArgs: {
		id: 'app.migrate-modrinth-app.settings.extra-launch-args',
		defaultMessage: 'Extra launch arguments',
	},
	settingsCustomEnvVars: {
		id: 'app.migrate-modrinth-app.settings.custom-env-vars',
		defaultMessage: 'Custom environment variables',
	},
	settingsMemoryMaximum: {
		id: 'app.migrate-modrinth-app.settings.memory-maximum',
		defaultMessage: 'Maximum memory allocation',
	},
	settingsForceFullscreen: {
		id: 'app.migrate-modrinth-app.settings.force-fullscreen',
		defaultMessage: 'Force fullscreen',
	},
	settingsGameResolution: {
		id: 'app.migrate-modrinth-app.settings.game-resolution',
		defaultMessage: 'Game resolution',
	},
	settingsHooks: {
		id: 'app.migrate-modrinth-app.settings.hooks',
		defaultMessage: 'Pre-launch, wrapper, and post-exit hooks',
	},
	javaVersionsTitle: {
		id: 'app.migrate-modrinth-app.java-versions-title',
		defaultMessage: 'Java installations',
	},
	importButton: {
		id: 'app.migrate-modrinth-app.import-button',
		defaultMessage: 'Import',
	},
	progressTitle: {
		id: 'app.migrate-modrinth-app.progress-title',
		defaultMessage: 'Import progress',
	},
	cancelAll: {
		id: 'app.migrate-modrinth-app.cancel-all',
		defaultMessage: 'Cancel all',
	},
	cancelJob: {
		id: 'app.migrate-modrinth-app.cancel-job',
		defaultMessage: 'Cancel',
	},
	categoryMods: { id: 'app.migrate-modrinth-app.category.mods', defaultMessage: 'Mods' },
	categoryResourcePacks: {
		id: 'app.migrate-modrinth-app.category.resource-packs',
		defaultMessage: 'Resource packs',
	},
	categoryShaderPacks: {
		id: 'app.migrate-modrinth-app.category.shader-packs',
		defaultMessage: 'Shader packs',
	},
	categoryConfig: { id: 'app.migrate-modrinth-app.category.config', defaultMessage: 'Config' },
	categorySaves: { id: 'app.migrate-modrinth-app.category.saves', defaultMessage: 'Worlds/saves' },
	categoryScreenshots: {
		id: 'app.migrate-modrinth-app.category.screenshots',
		defaultMessage: 'Screenshots',
	},
	categoryLogs: { id: 'app.migrate-modrinth-app.category.logs', defaultMessage: 'Logs' },
	symlinkTitle: {
		id: 'app.migrate-modrinth-app.symlink.title',
		defaultMessage: '{category} (linked folder)',
	},
	symlinkTargetInside: {
		id: 'app.migrate-modrinth-app.symlink.target-inside',
		defaultMessage:
			'This points inside the official Modrinth App itself - Link will copy its contents instead.',
	},
	symlinkTargetOutside: {
		id: 'app.migrate-modrinth-app.symlink.target-outside',
		defaultMessage:
			'Linked to {target} - Link keeps it there instead of copying it, so both installs would keep using the same files.',
	},
})

const categoryLabels: Record<ContentCategory, string> = {
	mods: formatMessage(messages.categoryMods),
	resource_packs: formatMessage(messages.categoryResourcePacks),
	shader_packs: formatMessage(messages.categoryShaderPacks),
	config: formatMessage(messages.categoryConfig),
	saves: formatMessage(messages.categorySaves),
	screenshots: formatMessage(messages.categoryScreenshots),
	logs: formatMessage(messages.categoryLogs),
}

const modal = ref<InstanceType<typeof NewModal> | null>(null)
const step = ref<Step>('detecting')
const errorMessage = ref('')
const manualPath = ref('')
const source = ref<DetectedSource | null>(null)
const preview = ref<ImportPreview | null>(null)

const includedInstances = reactive<Record<string, boolean>>({})
const expandedInstances = reactive<Record<string, boolean>>({})
const categorySelections = reactive<Record<string, Partial<Record<ContentCategory, boolean>>>>({})
const worldSelections = reactive<Record<string, Record<string, boolean>>>({})
/** Keyed by `[instance.sourceId][symlink.relativePath]`. */
const symlinkSelections = reactive<Record<string, Record<string, SymlinkAction>>>({})
/** Whether to carry over an instance's own launch overrides - only meaningful when it has any. */
const launchOverridesSelections = reactive<Record<string, boolean>>({})
const settingsSelection = reactive<SettingsImportSelection>({
	extraLaunchArgs: false,
	customEnvVars: false,
	memoryMaximum: false,
	forceFullscreen: false,
	gameResolution: false,
	hooks: false,
	javaVersions: false,
})

const activeJobs = reactive<Record<string, ActiveJob>>({})
let unlistenInstallJob: (() => void) | null = null

const allJobsFinished = computed(() => {
	const jobs = Object.values(activeJobs)
	return jobs.length > 0 && jobs.every((job) => isInstallJobFinished(job.snapshot.status))
})

defineExpose({
	show: () => {
		resetState()
		modal.value?.show()
		void start()
	},
})

function resetState() {
	step.value = 'detecting'
	errorMessage.value = ''
	manualPath.value = ''
	source.value = null
	preview.value = null
	for (const key of Object.keys(includedInstances)) Reflect.deleteProperty(includedInstances, key)
	for (const key of Object.keys(expandedInstances)) Reflect.deleteProperty(expandedInstances, key)
	for (const key of Object.keys(categorySelections)) Reflect.deleteProperty(categorySelections, key)
	for (const key of Object.keys(worldSelections)) Reflect.deleteProperty(worldSelections, key)
	for (const key of Object.keys(symlinkSelections)) Reflect.deleteProperty(symlinkSelections, key)
	for (const key of Object.keys(launchOverridesSelections))
		Reflect.deleteProperty(launchOverridesSelections, key)
	for (const key of Object.keys(activeJobs)) Reflect.deleteProperty(activeJobs, key)
	settingsSelection.extraLaunchArgs = false
	settingsSelection.customEnvVars = false
	settingsSelection.memoryMaximum = false
	settingsSelection.forceFullscreen = false
	settingsSelection.gameResolution = false
	settingsSelection.hooks = false
	settingsSelection.javaVersions = false
	unlistenInstallJob?.()
	unlistenInstallJob = null
}

async function start() {
	step.value = 'detecting'
	try {
		const detected = await detectModrinthAppInstall()
		if (!detected) {
			step.value = 'need-folder'
			return
		}
		await afterSourceFound(detected)
	} catch (error) {
		errorMessage.value = getErrorMessage(error)
		step.value = 'error'
	}
}

async function afterSourceFound(detected: DetectedSource) {
	source.value = detected
	try {
		if (await isModrinthAppRunning()) {
			step.value = 'running'
			return
		}
		await loadPreview(detected)
	} catch (error) {
		errorMessage.value = getErrorMessage(error)
		step.value = 'error'
	}
}

async function loadPreview(detected: DetectedSource) {
	step.value = 'detecting'
	try {
		const result = await previewModrinthAppImport(detected)
		preview.value = result
		initSelections(result)
		step.value = 'preview'
	} catch (error) {
		errorMessage.value = getErrorMessage(error)
		step.value = 'error'
	}
}

function initSelections(result: ImportPreview) {
	for (const instance of result.instances) {
		includedInstances[instance.sourceId] = !!instance.gameVersion
		expandedInstances[instance.sourceId] = false

		const categories: Partial<Record<ContentCategory, boolean>> = {}
		for (const category of instance.categories) {
			categories[category.category] = category.defaultSelected
		}
		categorySelections[instance.sourceId] = categories

		const worlds: Record<string, boolean> = {}
		const savesDefaultSelected = categories.saves ?? true
		for (const world of instance.worlds) {
			worlds[world.folderName] = savesDefaultSelected
		}
		worldSelections[instance.sourceId] = worlds

		const symlinks: Record<string, SymlinkAction> = {}
		for (const symlink of instance.symlinks) {
			symlinks[symlink.relativePath] = 'recreate'
		}
		symlinkSelections[instance.sourceId] = symlinks

		launchOverridesSelections[instance.sourceId] = instance.launchOverrides != null
	}

	settingsSelection.extraLaunchArgs = !!result.settings.extraLaunchArgs
	settingsSelection.customEnvVars = !!result.settings.customEnvVars
	settingsSelection.memoryMaximum = result.settings.memoryMaximumMb != null
	settingsSelection.forceFullscreen = result.settings.forceFullscreen != null
	settingsSelection.gameResolution = !!result.settings.gameResolution
	settingsSelection.hooks = !!(
		result.settings.hookPreLaunch ||
		result.settings.hookWrapper ||
		result.settings.hookPostExit
	)
	settingsSelection.javaVersions = result.javaVersions.length > 0
}

async function browseForFolder() {
	const path = await open({ multiple: false, directory: true })
	if (!path) return
	manualPath.value = path.toString()
	try {
		const detected = await detectModrinthAppInstall(manualPath.value)
		if (!detected) {
			errorMessage.value = formatMessage(messages.needFolderText)
			step.value = 'error'
			return
		}
		await afterSourceFound(detected)
	} catch (error) {
		errorMessage.value = getErrorMessage(error)
		step.value = 'error'
	}
}

function retry() {
	if (source.value) {
		void afterSourceFound(source.value)
	} else {
		void start()
	}
}

function toggleExpanded(sourceId: string) {
	expandedInstances[sourceId] = !expandedInstances[sourceId]
}

function toggleCategory(
	instance: ImportInstanceCandidate,
	category: ContentCategory,
	value: boolean,
) {
	categorySelections[instance.sourceId][category] = value
	if (category === 'saves') {
		const worlds = worldSelections[instance.sourceId]
		for (const folderName of Object.keys(worlds)) worlds[folderName] = value
	}
}

function formatModified(unixSeconds: number | null): string {
	if (unixSeconds == null) return ''
	return new Date(unixSeconds * 1000).toLocaleDateString()
}

/** Whole-category symlinks only - a symlinked world is looked up with `symlinkForWorld`. */
function categorySymlinks(instance: ImportInstanceCandidate): ImportSymlinkCandidate[] {
	return instance.symlinks.filter((symlink) => symlink.category !== 'saves')
}

function symlinkForWorld(
	instance: ImportInstanceCandidate,
	folderName: string,
): ImportSymlinkCandidate | undefined {
	return instance.symlinks.find((symlink) => symlink.relativePath === `saves/${folderName}`)
}

function setSymlinkAction(
	instance: ImportInstanceCandidate,
	relativePath: string,
	action: SymlinkAction,
) {
	symlinkSelections[instance.sourceId][relativePath] = action
}

const isImporting = ref(false)

function formatJobStatus(snapshot: InstallJobSnapshot): string {
	if (snapshot.status === 'running') return snapshot.phase.replaceAll('_', ' ')
	return snapshot.status
}

function progressRatio(snapshot: InstallJobSnapshot): number | null {
	if (!snapshot.progress || snapshot.progress.total <= 0) return null
	return snapshot.progress.current / snapshot.progress.total
}

async function cancelJob(jobId: string) {
	try {
		activeJobs[jobId].snapshot = await install_job_cancel(jobId)
	} catch (error) {
		handleError(error)
	}
}

function cancelAll() {
	for (const [jobId, job] of Object.entries(activeJobs)) {
		if (!isInstallJobFinished(job.snapshot.status)) void cancelJob(jobId)
	}
}

async function confirmImport() {
	if (!preview.value || isImporting.value) return
	isImporting.value = true
	step.value = 'importing'

	unlistenInstallJob = appEvents.on('install_job', (snapshot) => {
		const job = activeJobs[snapshot.job_id]
		if (job) job.snapshot = snapshot
	})

	for (const instance of preview.value.instances) {
		if (!includedInstances[instance.sourceId] || !instance.gameVersion) continue

		const categories = (
			Object.keys(categorySelections[instance.sourceId]) as ContentCategory[]
		).filter((category) => categorySelections[instance.sourceId][category])
		const worlds: string[] = []
		const symlinkActions: Record<string, SymlinkAction> = {}

		// Whole-category symlinks never appear in `instance.categories` (see
		// `ImportInstanceCandidate.categories`'s doc comment), so they aren't
		// covered by the loop above - resolve them from their own 3-way
		// choice instead, translating "recreate" to "copy" when the link's
		// target is inside the source app (see `SymlinkAction`).
		for (const symlink of categorySymlinks(instance)) {
			const chosen = symlinkSelections[instance.sourceId][symlink.relativePath]
			const resolved: SymlinkAction =
				chosen === 'recreate' && !symlink.targetOutsideSourceApp ? 'copy' : chosen
			symlinkActions[symlink.relativePath] = resolved
			if (resolved !== 'ignore') categories.push(symlink.category)
		}

		for (const world of instance.worlds) {
			const symlink = symlinkForWorld(instance, world.folderName)
			if (symlink) {
				const chosen = symlinkSelections[instance.sourceId][symlink.relativePath]
				const resolved: SymlinkAction =
					chosen === 'recreate' && !symlink.targetOutsideSourceApp ? 'copy' : chosen
				symlinkActions[symlink.relativePath] = resolved
				if (resolved !== 'ignore') worlds.push(world.folderName)
			} else if (worldSelections[instance.sourceId][world.folderName]) {
				worlds.push(world.folderName)
			}
		}

		const selection: ImportSelection = { categories, worlds, symlinkActions }

		try {
			const snapshot = await installImportModrinthAppInstance({
				sourceInstanceDir: instance.instanceDir,
				name: instance.name,
				gameVersion: instance.gameVersion,
				loader: instance.loader,
				loaderVersion: instance.loaderVersion,
				iconPath: instance.iconPath,
				selection,
				deleteSourceAfterImport: false,
				lastPlayed: instance.lastPlayed,
				totalTimePlayed: instance.totalTimePlayed,
				launchOverrides: launchOverridesSelections[instance.sourceId]
					? instance.launchOverrides
					: null,
			})
			activeJobs[snapshot.job_id] = { instanceName: instance.name, snapshot }
		} catch (error) {
			handleError(error)
		}
	}

	const wantsSettings = Object.values(settingsSelection).some(Boolean)
	if (wantsSettings) {
		try {
			await applyModrinthAppSettings(preview.value.settings, preview.value.javaVersions, {
				...settingsSelection,
			})
		} catch (error) {
			handleError(error)
		}
	}

	isImporting.value = false

	if (Object.keys(activeJobs).length === 0) {
		modal.value?.hide()
	}
}
</script>

<template>
	<NewModal ref="modal" :header="formatMessage(messages.header)" scrollable width="42rem">
		<div class="flex flex-col gap-4 min-h-[12rem]">
			<div
				v-if="step === 'detecting'"
				class="flex flex-1 items-center justify-center gap-2 py-12 text-secondary"
			>
				<LoaderCircleIcon class="size-5 animate-spin" />
				{{ formatMessage(messages.detecting) }}
			</div>

			<div
				v-else-if="step === 'need-folder'"
				class="flex flex-col items-center gap-3 py-8 text-center"
			>
				<TriangleAlertIcon class="size-8 text-orange" />
				<h2 class="m-0 text-lg font-semibold text-contrast">
					{{ formatMessage(messages.needFolderTitle) }}
				</h2>
				<p class="m-0 max-w-sm text-secondary">{{ formatMessage(messages.needFolderText) }}</p>
				<Button type="colored" color="brand" @click="browseForFolder">
					<FolderSearchIcon />
					{{ formatMessage(messages.browse) }}
				</Button>
			</div>

			<div v-else-if="step === 'running'" class="flex flex-col items-center gap-3 py-8 text-center">
				<TriangleAlertIcon class="size-8 text-orange" />
				<h2 class="m-0 text-lg font-semibold text-contrast">
					{{ formatMessage(messages.runningTitle) }}
				</h2>
				<p class="m-0 max-w-sm text-secondary">{{ formatMessage(messages.runningText) }}</p>
				<Button type="colored" color="brand" @click="retry">
					{{ formatMessage(messages.tryAgain) }}
				</Button>
			</div>

			<div v-else-if="step === 'error'" class="flex flex-col items-center gap-3 py-8 text-center">
				<TriangleAlertIcon class="size-8 text-red" />
				<h2 class="m-0 text-lg font-semibold text-contrast">
					{{ formatMessage(messages.errorTitle) }}
				</h2>
				<p class="m-0 max-w-sm text-secondary">{{ errorMessage }}</p>
				<div class="flex gap-2">
					<Button @click="browseForFolder">
						<FolderSearchIcon />
						{{ formatMessage(messages.browse) }}
					</Button>
					<Button type="colored" color="brand" @click="retry">
						{{ formatMessage(messages.tryAgain) }}
					</Button>
				</div>
			</div>

			<div v-else-if="step === 'importing'" class="flex flex-col gap-2">
				<h3 class="m-0 font-semibold text-contrast">
					{{ formatMessage(messages.progressTitle) }}
				</h3>
				<div
					v-for="(job, jobId) in activeJobs"
					:key="jobId"
					class="flex flex-col gap-2 rounded-2xl border border-solid border-surface-4 p-3"
				>
					<div class="flex items-center justify-between gap-2">
						<span class="font-semibold text-contrast truncate">{{ job.instanceName }}</span>
						<div class="flex items-center gap-2 shrink-0">
							<span class="text-sm text-secondary capitalize">
								{{ formatJobStatus(job.snapshot) }}
							</span>
							<IconButton
								v-if="!isInstallJobFinished(job.snapshot.status)"
								:label="formatMessage(messages.cancelJob)"
								size="sm"
								@click="cancelJob(jobId as string)"
							>
								<XCircleIcon class="size-4" />
							</IconButton>
						</div>
					</div>
					<ProgressBar
						v-if="progressRatio(job.snapshot) != null"
						:progress="progressRatio(job.snapshot)!"
					/>
				</div>
				<Button v-if="!allJobsFinished" type="outlined" color="red" @click="cancelAll">
					<XCircleIcon />
					{{ formatMessage(messages.cancelAll) }}
				</Button>
			</div>

			<template v-else-if="preview">
				<div class="flex flex-col gap-2">
					<h3 class="m-0 font-semibold text-contrast">
						{{ formatMessage(messages.instancesTitle) }}
					</h3>
					<div
						v-for="instance in preview.instances"
						:key="instance.sourceId"
						class="flex flex-col rounded-2xl border border-solid border-surface-4 overflow-clip"
					>
						<div class="flex w-full items-center gap-3 bg-surface-3 p-3">
							<IconButton
								:label="'Toggle'"
								:disabled="!instance.gameVersion"
								@click="toggleExpanded(instance.sourceId)"
							>
								<ChevronRightIcon
									class="size-5 transition-transform"
									:class="{ 'rotate-90': expandedInstances[instance.sourceId] }"
								/>
							</IconButton>
							<Checkbox
								:model-value="includedInstances[instance.sourceId]"
								:disabled="!instance.gameVersion"
								@update:model-value="includedInstances[instance.sourceId] = $event"
							/>
							<img
								v-if="instance.iconPath"
								:src="convertFileSrc(instance.iconPath)"
								alt=""
								class="size-8 rounded-md shrink-0"
							/>
							<div class="flex flex-col min-w-0">
								<span class="font-semibold text-contrast truncate">{{ instance.name }}</span>
								<span class="text-sm text-secondary truncate">
									<template v-if="instance.gameVersion">
										{{ instance.gameVersion }}
										<template v-if="instance.rawLoader">· {{ instance.rawLoader }}</template>
									</template>
									<template v-else>{{ formatMessage(messages.noGameVersion) }}</template>
								</span>
							</div>
						</div>
						<Collapsible :collapsed="!expandedInstances[instance.sourceId]">
							<div class="flex flex-col gap-2 p-3">
								<Checkbox
									v-if="instance.launchOverrides"
									v-model="launchOverridesSelections[instance.sourceId]"
									:label="formatMessage(messages.launchOverrides)"
								/>
								<div
									v-for="category in instance.categories"
									:key="category.category"
									class="flex flex-col gap-1"
								>
									<Checkbox
										:model-value="categorySelections[instance.sourceId][category.category]"
										@update:model-value="toggleCategory(instance, category.category, $event)"
									>
										<span class="text-sm">
											{{ categoryLabels[category.category] }}
											<span class="text-secondary">
												({{ category.fileCount }}
												<template v-if="category.totalSize != null">
													, {{ formatBytes(category.totalSize) }}</template
												>)
											</span>
										</span>
									</Checkbox>
									<div
										v-if="category.category === 'saves' && instance.worlds.length > 0"
										class="flex flex-col gap-1 pl-8"
									>
										<template v-for="world in instance.worlds" :key="world.folderName">
											<div
												v-if="symlinkForWorld(instance, world.folderName)"
												class="flex flex-col gap-1"
											>
												<div class="flex items-center justify-between gap-2">
													<span class="text-sm">
														{{ world.folderName }}
														<span v-if="world.modified" class="text-secondary">
															· {{ formatModified(world.modified) }}
														</span>
													</span>
													<SymlinkActionSelector
														:model-value="
															symlinkSelections[instance.sourceId][
																symlinkForWorld(instance, world.folderName)!.relativePath
															]
														"
														@update:model-value="
															setSymlinkAction(
																instance,
																symlinkForWorld(instance, world.folderName)!.relativePath,
																$event,
															)
														"
													/>
												</div>
												<p class="m-0 text-xs text-secondary">
													{{
														symlinkForWorld(instance, world.folderName)!.targetOutsideSourceApp
															? formatMessage(messages.symlinkTargetOutside, {
																	target: symlinkForWorld(instance, world.folderName)!.target,
																})
															: formatMessage(messages.symlinkTargetInside)
													}}
												</p>
											</div>
											<Checkbox
												v-else
												:model-value="worldSelections[instance.sourceId][world.folderName]"
												@update:model-value="
													worldSelections[instance.sourceId][world.folderName] = $event
												"
											>
												<span class="text-sm">
													{{ world.folderName }}
													<span v-if="world.modified" class="text-secondary">
														· {{ formatModified(world.modified) }}
													</span>
												</span>
											</Checkbox>
										</template>
									</div>
								</div>

								<div
									v-for="symlink in categorySymlinks(instance)"
									:key="symlink.relativePath"
									class="flex flex-col gap-1"
								>
									<div class="flex items-center justify-between gap-2">
										<span class="text-sm">
											{{
												formatMessage(messages.symlinkTitle, {
													category: categoryLabels[symlink.category],
												})
											}}
										</span>
										<SymlinkActionSelector
											:model-value="symlinkSelections[instance.sourceId][symlink.relativePath]"
											@update:model-value="setSymlinkAction(instance, symlink.relativePath, $event)"
										/>
									</div>
									<p class="m-0 text-xs text-secondary">
										{{
											symlink.targetOutsideSourceApp
												? formatMessage(messages.symlinkTargetOutside, {
														target: symlink.target,
													})
												: formatMessage(messages.symlinkTargetInside)
										}}
									</p>
								</div>
							</div>
						</Collapsible>
					</div>
				</div>

				<div class="flex flex-col gap-2">
					<h3 class="m-0 font-semibold text-contrast">
						{{ formatMessage(messages.settingsTitle) }}
					</h3>
					<div class="flex flex-col gap-2 rounded-2xl border border-solid border-surface-4 p-3">
						<Checkbox
							v-model="settingsSelection.extraLaunchArgs"
							:disabled="!preview.settings.extraLaunchArgs"
							:label="formatMessage(messages.settingsExtraLaunchArgs)"
						/>
						<Checkbox
							v-model="settingsSelection.customEnvVars"
							:disabled="!preview.settings.customEnvVars"
							:label="formatMessage(messages.settingsCustomEnvVars)"
						/>
						<Checkbox
							v-model="settingsSelection.memoryMaximum"
							:disabled="preview.settings.memoryMaximumMb == null"
							:label="formatMessage(messages.settingsMemoryMaximum)"
						/>
						<Checkbox
							v-model="settingsSelection.forceFullscreen"
							:disabled="preview.settings.forceFullscreen == null"
							:label="formatMessage(messages.settingsForceFullscreen)"
						/>
						<Checkbox
							v-model="settingsSelection.gameResolution"
							:disabled="!preview.settings.gameResolution"
							:label="formatMessage(messages.settingsGameResolution)"
						/>
						<Checkbox
							v-model="settingsSelection.hooks"
							:disabled="
								!preview.settings.hookPreLaunch &&
								!preview.settings.hookWrapper &&
								!preview.settings.hookPostExit
							"
							:label="formatMessage(messages.settingsHooks)"
						/>
						<Checkbox
							v-model="settingsSelection.javaVersions"
							:disabled="preview.javaVersions.length === 0"
						>
							<span class="flex items-center gap-1 text-sm">
								<CoffeeIcon class="size-4" />
								{{ formatMessage(messages.javaVersionsTitle) }} ({{ preview.javaVersions.length }})
							</span>
						</Checkbox>
					</div>
				</div>
			</template>
		</div>
		<template #actions>
			<div class="flex items-center justify-end gap-2">
				<Button type="outlined" @click="modal?.hide()">
					<XIcon />
					{{
						formatMessage(
							step === 'importing' ? commonMessages.closeButton : commonMessages.cancelButton,
						)
					}}
				</Button>
				<Button
					v-if="step === 'preview'"
					type="colored"
					color="brand"
					:disabled="isImporting"
					@click="confirmImport"
				>
					<ImportIcon />
					{{ formatMessage(messages.importButton) }}
				</Button>
			</div>
		</template>
	</NewModal>
</template>
