<script setup lang="ts">
import {
	CoffeeIcon,
	GaugeIcon,
	ImportIcon,
	LanguagesIcon,
	ModrinthIcon,
	PaintbrushIcon,
	RefreshCwIcon,
	Settings2Icon,
	ToggleRightIcon,
} from '@modrinth/assets'
import {
	Button,
	commonMessages,
	commonSettingsMessages,
	defineMessage,
	defineMessages,
	injectNotificationManager,
	TabbedModal,
	UnsavedChangesPopup,
	useVIntl,
} from '@modrinth/ui'
import { getVersion } from '@tauri-apps/api/app'
import { openUrl } from '@tauri-apps/plugin-opener'
import { platform as getOsPlatform, version as getOsVersion } from '@tauri-apps/plugin-os'
import { computed, provide, ref, watch } from 'vue'

import AppearanceSettings from '@/components/ui/settings/display/AppearanceSettings.vue'
import BehaviorSettings from '@/components/ui/settings/display/BehaviorSettings.vue'
import FeatureFlagSettings from '@/components/ui/settings/display/FeatureFlagSettings.vue'
import LanguageSettings from '@/components/ui/settings/display/LanguageSettings.vue'
import InstancesSyncedSettings from '@/components/ui/settings/instances/InstancesSyncedSettings.vue'
import JavaSettings from '@/components/ui/settings/instances/JavaSettings.vue'
import MigrateModrinthAppSettings from '@/components/ui/settings/instances/MigrateModrinthAppSettings.vue'
import ResourceManagementSettings from '@/components/ui/settings/instances/ResourceManagementSettings.vue'
import { useAppSettings } from '@/composables/use-app-settings.ts'
import { get, set } from '@/helpers/settings.ts'
import { copyToClipboard } from '@/helpers/utils.js'
import {
	appSettingsModalContextKey,
	type UnsavedChangesController,
} from '@/providers/app-settings-modal'
import { appUpdateCheck, checkForAppUpdate, RELEASES_PAGE_URL } from '@/providers/app-update.ts'

// TODO: Apply COMPONENT_STRUCTURE.md here and extract out common setting option components
const appSettings = useAppSettings()

const { formatMessage } = useVIntl()

const devModeCounter = ref(0)

const developerModeEnabled = defineMessage({
	id: 'app.settings.developer-mode-enabled',
	defaultMessage: 'Developer mode enabled.',
})

const tabCategories = defineMessages({
	display: {
		id: 'settings.sidebar.label.display',
		defaultMessage: 'Display',
	},
	instances: {
		id: 'app.settings.sidebar.label.instances',
		defaultMessage: 'Instances',
	},
})

const tabs = [
	{
		name: defineMessage({
			id: 'app.settings.tabs.appearance',
			defaultMessage: 'Appearance',
		}),
		category: tabCategories.display,
		icon: PaintbrushIcon,
		content: AppearanceSettings,
	},
	{
		name: defineMessage({
			id: 'app.settings.tabs.behavior',
			defaultMessage: 'Behavior',
		}),
		category: tabCategories.display,
		icon: Settings2Icon,
		content: BehaviorSettings,
	},
	{
		name: defineMessage({
			id: 'app.settings.tabs.language',
			defaultMessage: 'Language',
		}),
		category: tabCategories.display,
		icon: LanguagesIcon,
		content: LanguageSettings,
		badge: commonMessages.beta,
	},
	{
		name: commonSettingsMessages.featureFlags,
		category: tabCategories.display,
		icon: ToggleRightIcon,
		content: FeatureFlagSettings,
		developerOnly: true,
	},
	{
		name: defineMessage({
			id: 'app.settings.tabs.synced-options',
			defaultMessage: 'Synced settings',
		}),
		category: tabCategories.instances,
		icon: RefreshCwIcon,
		content: InstancesSyncedSettings,
	},
	{
		name: defineMessage({
			id: 'app.settings.tabs.java-installations',
			defaultMessage: 'Java installations',
		}),
		category: tabCategories.instances,
		icon: CoffeeIcon,
		content: JavaSettings,
	},
	{
		name: defineMessage({
			id: 'app.settings.tabs.resource-management',
			defaultMessage: 'Resource management',
		}),
		category: tabCategories.instances,
		icon: GaugeIcon,
		content: ResourceManagementSettings,
	},
	{
		name: defineMessage({
			id: 'app.settings.tabs.migrate-modrinth-app',
			defaultMessage: 'Import from Modrinth App',
		}),
		category: tabCategories.instances,
		icon: ImportIcon,
		content: MigrateModrinthAppSettings,
	},
]

const availableTabs = computed(() =>
	tabs.filter((tab) => !tab.developerOnly || appSettings.devMode),
)

const modal = ref<InstanceType<typeof TabbedModal> | null>(null)
const unsavedChangesPopup = ref<{ nudge: () => void } | null>(null)
const unsavedChangesController = ref<UnsavedChangesController | null>(null)
const emptyUnsavedChangesState: Record<string, unknown> = {}
const originalUnsavedChangesState = computed(
	() => unsavedChangesController.value?.getOriginal() ?? emptyUnsavedChangesState,
)
const modifiedUnsavedChangesState = computed(
	() => unsavedChangesController.value?.getModified() ?? emptyUnsavedChangesState,
)
const savingUnsavedChanges = computed(() => unsavedChangesController.value?.isSaving() ?? false)
const hasUnsavedChanges = computed(
	() =>
		(unsavedChangesController.value?.hasChanges() ?? false) ||
		(unsavedChangesController.value?.isSaving() ?? false),
)

function canLeaveCurrentTab(): boolean {
	if (
		!unsavedChangesController.value?.hasChanges() &&
		!unsavedChangesController.value?.isSaving()
	) {
		return true
	}
	unsavedChangesPopup.value?.nudge()
	return false
}

function close(): boolean {
	return modal.value?.hide() ?? false
}

function registerUnsavedChangesController(controller: UnsavedChangesController | null): void {
	unsavedChangesController.value = controller
}

provide(appSettingsModalContextKey, {
	close,
	registerUnsavedChangesController,
})

function resetUnsavedChanges(): void {
	unsavedChangesController.value?.reset()
}

function saveUnsavedChanges(): void {
	void unsavedChangesController.value?.save()
}

function show() {
	modal.value?.show()
}

function showFeatureFlags(): void {
	const featureFlagsTabIndex = availableTabs.value.findIndex(
		(tab) => tab.content === FeatureFlagSettings,
	)
	if (featureFlagsTabIndex >= 0) {
		modal.value?.setTab(featureFlagsTabIndex)
	}
	modal.value?.show()
}

function showSyncedOptions(): void {
	const syncedOptionsTabIndex = availableTabs.value.findIndex(
		(tab) => tab.content === InstancesSyncedSettings,
	)
	if (syncedOptionsTabIndex >= 0) {
		modal.value?.setTab(syncedOptionsTabIndex)
	}
	modal.value?.show()
}

defineExpose({ show, showFeatureFlags, showSyncedOptions })

const { handleError, addNotification } = injectNotificationManager()

const { checking, updateAvailable, latestVersion, downloadUrl, lastCheckedAt } = appUpdateCheck

async function openUpdateDownload() {
	const target = downloadUrl.value ?? RELEASES_PAGE_URL
	console.log('Opening app update download URL:', target)
	try {
		await openUrl(target)
		console.log('openUrl resolved without throwing for:', target)
	} catch (error) {
		console.error('openUrl failed for update download:', target, error)
		try {
			await copyToClipboard(target)
			addNotification({
				title: formatMessage(messages.linkCopiedTitle),
				text: formatMessage(messages.linkCopiedText),
				type: 'info',
			})
		} catch {
			handleError(error)
		}
	}
}

const version = await getVersion()
const osPlatform = getOsPlatform()
const osVersion = getOsVersion()
const settings = ref(await get())

watch(
	settings,
	async () => {
		await set(settings.value)
	},
	{ deep: true },
)

function devModeCount() {
	devModeCounter.value++
	if (devModeCounter.value > 5) {
		const selectedTab = modal.value ? availableTabs.value[modal.value.selectedTab] : undefined

		appSettings.devMode = !appSettings.devMode
		settings.value.developer_mode = !!appSettings.devMode
		devModeCounter.value = 0

		if (modal.value) {
			const selectedTabIndex = selectedTab ? availableTabs.value.indexOf(selectedTab) : -1
			modal.value.setTab(selectedTabIndex >= 0 ? selectedTabIndex : 0)
		}
	}
}

const messages = defineMessages({
	appVersion: {
		id: 'app.settings.app-version',
		defaultMessage: 'Dyad Launcher {version}',
	},
	macos: {
		id: 'app.settings.operating-system.macos',
		defaultMessage: 'macOS',
	},
	developerModeButtonLabel: {
		id: 'app.settings.developer-mode-button.label',
		defaultMessage: 'Toggle developer mode',
	},
	checkForUpdates: {
		id: 'app.settings.check-for-updates',
		defaultMessage: 'Check for updates',
	},
	checking: {
		id: 'app.settings.check-for-updates.checking',
		defaultMessage: 'Checking...',
	},
	upToDate: {
		id: 'app.settings.check-for-updates.up-to-date',
		defaultMessage: "You're up to date",
	},
	updateAvailable: {
		id: 'app.settings.check-for-updates.update-available',
		defaultMessage: 'v{version} is available',
	},
	download: {
		id: 'app.settings.check-for-updates.download',
		defaultMessage: 'Download',
	},
	linkCopiedTitle: {
		id: 'app.settings.check-for-updates.link-copied.title',
		defaultMessage: 'Could not open the download link',
	},
	linkCopiedText: {
		id: 'app.settings.check-for-updates.link-copied.text',
		defaultMessage: 'The link was copied to your clipboard instead - paste it into a browser.',
	},
})
</script>
<template>
	<TabbedModal
		ref="modal"
		:tabs="availableTabs"
		:width="'min(928px, calc(95vw - 10rem))'"
		:before-hide="canLeaveCurrentTab"
		:before-tab-change="canLeaveCurrentTab"
		:floating-action-bar-shown="hasUnsavedChanges"
	>
		<template #title>
			<span class="text-2xl font-semibold text-contrast">
				{{ formatMessage(commonMessages.settingsLabel) }}
			</span>
		</template>
		<template #floating-action-bar>
			<UnsavedChangesPopup
				ref="unsavedChangesPopup"
				:original="originalUnsavedChangesState"
				:modified="modifiedUnsavedChangesState"
				:saving="savingUnsavedChanges"
				inline
				@reset="resetUnsavedChanges"
				@save="saveUnsavedChanges"
			/>
		</template>
		<template #footer>
			<div class="mt-auto text-secondary text-sm">
				<p v-if="appSettings.devMode" class="text-brand font-semibold m-0 mb-2">
					{{ formatMessage(developerModeEnabled) }}
				</p>
				<div class="flex items-center gap-3">
					<button
						:aria-label="formatMessage(messages.developerModeButtonLabel)"
						class="p-0 m-0 bg-transparent border-none cursor-pointer button-animation"
						:class="{
							'text-brand': appSettings.devMode,
							'text-secondary': !appSettings.devMode,
						}"
						@click="devModeCount"
					>
						<ModrinthIcon aria-hidden="true" class="w-6 h-6" />
					</button>
					<div class="max-w-[200px]">
						<p class="m-0">
							{{ formatMessage(messages.appVersion, { version }) }}
						</p>
						<p class="m-0">
							<span v-if="osPlatform === 'macos'">{{ formatMessage(messages.macos) }}</span>
							<span v-else class="capitalize">{{ osPlatform }}</span>
							{{ osVersion }}
						</p>
					</div>
				</div>
				<div class="mt-3 flex items-center gap-2">
					<template v-if="updateAvailable">
						<span class="text-brand font-medium">
							{{ formatMessage(messages.updateAvailable, { version: latestVersion }) }}
						</span>
						<Button type="colored" color="brand" size="sm" @click="openUpdateDownload">
							{{ formatMessage(messages.download) }}
						</Button>
					</template>
					<template v-else>
						<span v-if="checking">{{ formatMessage(messages.checking) }}</span>
						<span v-else-if="lastCheckedAt">{{ formatMessage(messages.upToDate) }}</span>
						<Button type="outlined" size="sm" :disabled="checking" @click="checkForAppUpdate">
							{{ formatMessage(messages.checkForUpdates) }}
						</Button>
					</template>
				</div>
			</div>
		</template>
	</TabbedModal>
</template>
