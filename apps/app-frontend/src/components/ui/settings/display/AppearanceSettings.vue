<script setup lang="ts">
import {
	AppearanceSettingsLayout,
	defineMessages,
	provideAppearanceSettings,
	useSavable,
	useVIntl,
} from '@modrinth/ui'
import { computed, inject, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import BackgroundEditor from '@/components/ui/settings/display/BackgroundEditor.vue'
import { cloneBackground, setGlobalBackground } from '@/composables/use-background'
import { type ColorTheme, isDarkTheme, useTheme } from '@/composables/use-theme.ts'
import { type AppSettings, get, set } from '@/helpers/settings.ts'
import type { BackgroundConfig } from '@/helpers/types'
import { getOS } from '@/helpers/utils'
import { appSettingsModalContextKey } from '@/providers/app-settings-modal'

const { formatMessage } = useVIntl()
const messages = defineMessages({
	backgroundTitle: {
		id: 'app.settings.background.title',
		defaultMessage: 'Background',
	},
	backgroundDescription: {
		id: 'app.settings.background.description',
		defaultMessage:
			'Show your own image, color or gradient behind the launcher. Each instance can override this in its own settings.',
	},
})

const theme = useTheme()
const settingsModal = inject(appSettingsModalContextKey, null)
const os = await getOS()
const settings = ref(await get())

type AppearanceSettingsState = {
	theme: ColorTheme
	advancedRendering: boolean
	nativeDecorations: boolean
	background: BackgroundConfig
}

function getAppearanceSettingsState(settings: AppSettings): AppearanceSettingsState {
	return {
		theme: settings.theme,
		advancedRendering: settings.advanced_rendering,
		nativeDecorations: settings.native_decorations,
		background: settings.background,
	}
}

const { saved, current, changes, saving, hasChanges, reset, save } = useSavable(
	() => getAppearanceSettingsState(settings.value),
	async () => {
		const value = current.value

		const nextSettings: AppSettings = {
			...settings.value,
			theme: value.theme,
			advanced_rendering: value.advancedRendering,
			native_decorations: value.nativeDecorations,
			background: value.background,
		}

		await set(nextSettings)
		settings.value = nextSettings
		if (isDarkTheme(value.theme)) {
			theme.preferredDark = value.theme
		}
		theme.preferred = value.theme
		theme.advancedRendering = value.advancedRendering
	},
)

const themeOptions = computed(() =>
	theme.options.filter(
		(option) =>
			option !== 'retro' || settings.value.developer_mode || current.value.theme === 'retro',
	),
)

const preferredDarkTheme = computed(() =>
	isDarkTheme(current.value.theme) ? current.value.theme : theme.preferredDark,
)

function setTheme(value: ColorTheme): void {
	current.value.theme = value
}

function setAdvancedRendering(enabled: boolean): void {
	current.value.advancedRendering = enabled
}

function setNativeDecorations(enabled: boolean): void {
	current.value.nativeDecorations = enabled
}

watch(
	[() => current.value.theme, () => saved.value.theme],
	([selectedTheme, savedTheme]) => {
		theme.preview = selectedTheme === savedTheme ? null : selectedTheme
	},
	{ immediate: true },
)

watch(
	() => current.value.background,
	(background) => setGlobalBackground(cloneBackground(background)),
	{ deep: true },
)

async function saveAppearanceSettings(): Promise<void> {
	try {
		await save()
	} catch {
		return
	}
}

onMounted(() => {
	settingsModal?.registerUnsavedChangesController({
		hasChanges: () => hasChanges.value,
		getOriginal: () => saved.value,
		getModified: () => changes.value,
		isSaving: () => saving.value,
		reset,
		save: saveAppearanceSettings,
	})
})

onBeforeUnmount(() => {
	theme.preview = null
	setGlobalBackground(cloneBackground(saved.value.background))
	settingsModal?.registerUnsavedChangesController(null)
})

provideAppearanceSettings({
	deferPersistence: true,
	theme: {
		current: computed(() => current.value.theme),
		options: themeOptions,
		system: computed(() => (theme.native === 'light' ? 'light' : preferredDarkTheme.value)),
		preferredDark: preferredDarkTheme,
		set: setTheme,
	},
	advancedRendering: {
		value: computed(() => current.value.advancedRendering),
		set: setAdvancedRendering,
	},
	nativeDecorations:
		os !== 'MacOS'
			? {
					value: computed(() => current.value.nativeDecorations),
					set: setNativeDecorations,
				}
			: undefined,
})
</script>

<template>
	<div>
		<AppearanceSettingsLayout />

		<section class="mt-8 border-0 border-t border-solid border-divider pt-6">
			<div class="flex flex-col gap-1">
				<h2 class="m-0 text-xl font-semibold text-contrast">
					{{ formatMessage(messages.backgroundTitle) }}
				</h2>
				<p class="m-0 text-secondary">
					{{ formatMessage(messages.backgroundDescription) }}
				</p>
			</div>
			<BackgroundEditor v-model="current.background" class="mt-4" />
		</section>
	</div>
</template>
