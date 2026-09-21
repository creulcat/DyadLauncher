<script setup lang="ts">
import { Chips, defineMessages, injectNotificationManager, useVIntl } from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { computed, onBeforeUnmount, onMounted, ref, watch, watchEffect } from 'vue'

import BackgroundEditor from '@/components/ui/settings/display/BackgroundEditor.vue'
import {
	cloneBackground,
	DEFAULT_BACKGROUND,
	sameBackground,
	setInstanceBackgroundPreview,
	useGlobalBackground,
} from '@/composables/use-background'
import { edit } from '@/helpers/instance'
import type { BackgroundConfig, BackgroundSource } from '@/helpers/types'

import { instanceKeys } from '../../query-options'
import { injectInstanceSettings } from './instance-settings-context'

type Mode = 'inherit' | 'none' | 'custom'
type Draft = { mode: Mode; config: BackgroundConfig }

const MODES: Mode[] = ['inherit', 'none', 'custom']
const CUSTOM_SOURCES: BackgroundSource['type'][] = ['image', 'color', 'gradient']

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const queryClient = useQueryClient()
const { instance, registerUnsavedChangesController } = injectInstanceSettings()
const globalBackground = useGlobalBackground()

const saved = computed<BackgroundConfig | null>(() => instance.value.background ?? null)

function draftFrom(background: BackgroundConfig | null): Draft {
	if (!background) return { mode: 'inherit', config: cloneBackground(DEFAULT_BACKGROUND) }
	return {
		mode: background.source.type === 'none' ? 'none' : 'custom',
		config: cloneBackground(background),
	}
}

/** What is being edited. Nothing is stored until Save; until then it is only previewed. */
const draft = ref<Draft>(draftFrom(saved.value))
const saving = ref(false)

/** The background the draft would store: `null` inherits the launcher background. */
const desired = computed<BackgroundConfig | null>(() => {
	const { mode, config } = draft.value
	if (mode === 'inherit') return null
	return mode === 'none' ? { ...config, source: { type: 'none' } } : config
})

const hasChanges = computed(() => !sameBackground(desired.value, saved.value))

/**
 * Starts a custom background from the launcher-wide one, so it changes something recognisable
 * instead of jumping to an unrelated default. With no global background it starts as the brand
 * gradient.
 */
function startingCustomConfig(): BackgroundConfig {
	const global = cloneBackground(globalBackground.value)
	if (global.source.type !== 'none') return global
	return { ...global, source: { type: 'gradient', from: '#54ff54', to: '#55ffff', angle: 135 } }
}

function selectMode(next: Mode | null | undefined) {
	if (!next || next === draft.value.mode) return

	if (next === 'custom' && draft.value.config.source.type === 'none') {
		draft.value = { mode: next, config: startingCustomConfig() }
		return
	}
	draft.value = { ...draft.value, mode: next }
}

function updateConfig(config: BackgroundConfig) {
	draft.value = { ...draft.value, config }
}

function reset() {
	draft.value = draftFrom(saved.value)
}

async function save() {
	if (!hasChanges.value || saving.value) return

	const instanceId = instance.value.id
	const value = desired.value ? cloneBackground(desired.value) : null

	saving.value = true
	try {
		await edit(instanceId, { background: value })
		await Promise.all([
			queryClient.invalidateQueries({ queryKey: instanceKeys.detail(instanceId) }),
			queryClient.invalidateQueries({ queryKey: instanceKeys.list() }),
		])
	} catch (error) {
		handleError(error)
	} finally {
		saving.value = false
	}
}

watchEffect(() => {
	setInstanceBackgroundPreview(
		hasChanges.value
			? {
					instanceId: instance.value.id,
					background: desired.value ? cloneBackground(desired.value) : null,
				}
			: null,
	)
})

watch(
	() => instance.value.id,
	() => reset(),
)

watch(saved, (next, previous) => {
	if (sameBackground(desired.value, previous)) draft.value = draftFrom(next)
})

onMounted(() => {
	registerUnsavedChangesController({
		hasChanges: () => hasChanges.value,
		getOriginal: () => ({ background: saved.value }),
		getModified: () => ({ background: desired.value }),
		isSaving: () => saving.value,
		reset,
		save,
	})
})

onBeforeUnmount(() => {
	registerUnsavedChangesController(null)
	setInstanceBackgroundPreview(null)
})

function formatMode(value: Mode) {
	switch (value) {
		case 'inherit':
			return formatMessage(messages.modeInherit)
		case 'none':
			return formatMessage(messages.modeNone)
		case 'custom':
			return formatMessage(messages.modeCustom)
	}
}

const messages = defineMessages({
	title: {
		id: 'instance.settings.tabs.general.background',
		defaultMessage: 'Background',
	},
	description: {
		id: 'instance.settings.tabs.general.background.description',
		defaultMessage:
			"Choose the background shown on this instance's pages. This does not change the game.",
	},
	modeInherit: {
		id: 'instance.settings.tabs.general.background.mode.inherit',
		defaultMessage: 'Use launcher background',
	},
	modeNone: {
		id: 'instance.settings.tabs.general.background.mode.none',
		defaultMessage: 'None',
	},
	modeCustom: {
		id: 'instance.settings.tabs.general.background.mode.custom',
		defaultMessage: 'Custom',
	},
})
</script>

<template>
	<div class="flex flex-col gap-2.5">
		<h2 id="instance-background-label" class="m-0 text-lg font-semibold text-contrast">
			{{ formatMessage(messages.title) }}
		</h2>
		<p class="m-0">{{ formatMessage(messages.description) }}</p>
		<Chips
			:model-value="draft.mode"
			:items="MODES"
			:format-label="formatMode"
			:capitalize="false"
			:aria-label="formatMessage(messages.title)"
			@update:model-value="selectMode"
		/>
		<BackgroundEditor
			v-if="draft.mode === 'custom'"
			class="mt-2"
			:model-value="draft.config"
			:sources="CUSTOM_SOURCES"
			@update:model-value="updateConfig"
		/>
	</div>
</template>
