<script setup lang="ts">
import { FolderOpenIcon } from '@modrinth/assets'
import {
	Button,
	Chips,
	defineMessages,
	injectNotificationManager,
	Slider,
	useVIntl,
} from '@modrinth/ui'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, ref, watch } from 'vue'

import AppBackground from '@/components/ui/AppBackground.vue'
import { useTheme } from '@/composables/use-theme'
import { cacheBackgroundImage, getBackgroundImageUrl } from '@/helpers/background'
import type { BackgroundConfig, BackgroundSource } from '@/helpers/types'

type SourceType = BackgroundSource['type']

const props = withDefaults(
	defineProps<{
		modelValue: BackgroundConfig
		sources?: SourceType[]
	}>(),
	{ sources: () => ['none', 'image', 'color', 'gradient'] },
)
const emit = defineEmits<{ 'update:modelValue': [BackgroundConfig] }>()

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const theme = useTheme()

const DEFAULT_COLOR = '#2b3a67'
const DEFAULT_GRADIENT = { from: '#54ff54', to: '#55ffff', angle: 135 }
const MAX_DIM_PANEL_OPACITY = 95
const MIN_PANEL_OPACITY = 40

/**
 * Choosing "Image" before a file is picked has no source to store yet, so the chosen type is
 * tracked separately from the stored source until an image arrives.
 */
const selectedType = ref<SourceType>(props.modelValue.source.type)
watch(
	() => props.modelValue.source.type,
	(type) => {
		selectedType.value = type
	},
)

const source = computed(() => props.modelValue.source)
const colorValue = computed(() =>
	source.value.type === 'color' ? source.value.color : DEFAULT_COLOR,
)
const gradientValue = computed(() => (source.value.type === 'gradient' ? source.value : null))
const imagePath = computed(() => (source.value.type === 'image' ? source.value.path : null))
const imageUrl = computed(() => (imagePath.value ? getBackgroundImageUrl(imagePath.value) : null))
const choosingImage = ref(false)

const panelOpacity = computed(() =>
	Math.round(
		MIN_PANEL_OPACITY + props.modelValue.dim * ((MAX_DIM_PANEL_OPACITY - MIN_PANEL_OPACITY) / 100),
	),
)
const blurDisabled = computed(() => !theme.advancedRendering)

function update(patch: Partial<BackgroundConfig>) {
	emit('update:modelValue', { ...props.modelValue, ...patch })
}

function setSource(next: BackgroundSource) {
	update({ source: next })
}

function selectType(type: SourceType | null | undefined) {
	if (!type) return
	selectedType.value = type
	switch (type) {
		case 'none':
			setSource({ type: 'none' })
			break
		case 'color':
			setSource({ type: 'color', color: colorValue.value })
			break
		case 'gradient':
			setSource({ type: 'gradient', ...(gradientValue.value ?? DEFAULT_GRADIENT) })
			break
		case 'image':
			if (imagePath.value) setSource({ type: 'image', path: imagePath.value })
			break
	}
}

function updateGradient(patch: Partial<typeof DEFAULT_GRADIENT>) {
	setSource({ type: 'gradient', ...(gradientValue.value ?? DEFAULT_GRADIENT), ...patch })
}

async function chooseImage() {
	const value = await open({
		multiple: false,
		directory: false,
		filters: [{ name: 'Image', extensions: ['png', 'jpg', 'jpeg', 'webp'] }],
	})
	if (!value) return

	choosingImage.value = true
	try {
		setSource({ type: 'image', path: await cacheBackgroundImage(value) })
	} catch (error) {
		handleError(error)
	} finally {
		choosingImage.value = false
	}
}

function formatSourceType(type: SourceType) {
	switch (type) {
		case 'none':
			return formatMessage(messages.sourceNone)
		case 'image':
			return formatMessage(messages.sourceImage)
		case 'color':
			return formatMessage(messages.sourceColor)
		case 'gradient':
			return formatMessage(messages.sourceGradient)
	}
}

const messages = defineMessages({
	sourceLabel: {
		id: 'app.settings.background.source',
		defaultMessage: 'Background type',
	},
	sourceNone: {
		id: 'app.settings.background.source.none',
		defaultMessage: 'None',
	},
	sourceImage: {
		id: 'app.settings.background.source.image',
		defaultMessage: 'Image',
	},
	sourceColor: {
		id: 'app.settings.background.source.color',
		defaultMessage: 'Color',
	},
	sourceGradient: {
		id: 'app.settings.background.source.gradient',
		defaultMessage: 'Gradient',
	},
	chooseImage: {
		id: 'app.settings.background.choose-image',
		defaultMessage: 'Choose image…',
	},
	changeImage: {
		id: 'app.settings.background.change-image',
		defaultMessage: 'Change image…',
	},
	imageHint: {
		id: 'app.settings.background.image-hint',
		defaultMessage:
			'PNG, JPEG or WebP. The image is copied into the launcher, so moving or deleting the original will not affect it.',
	},
	color: {
		id: 'app.settings.background.color',
		defaultMessage: 'Color',
	},
	gradientFrom: {
		id: 'app.settings.background.gradient-from',
		defaultMessage: 'From',
	},
	gradientTo: {
		id: 'app.settings.background.gradient-to',
		defaultMessage: 'To',
	},
	gradientAngle: {
		id: 'app.settings.background.gradient-angle',
		defaultMessage: 'Angle',
	},
	dim: {
		id: 'app.settings.background.dim',
		defaultMessage: 'Dim',
	},
	dimDescription: {
		id: 'app.settings.background.dim-description',
		defaultMessage:
			'Darkens the background and makes panels more solid, so text is easier to read.',
	},
	blur: {
		id: 'app.settings.background.blur',
		defaultMessage: 'Blur',
	},
	blurDescription: {
		id: 'app.settings.background.blur-description',
		defaultMessage: 'Softens the background image.',
	},
	blurDisabledDescription: {
		id: 'app.settings.background.blur-disabled-description',
		defaultMessage: 'Turn on advanced rendering to blur the background.',
	},
	preview: {
		id: 'app.settings.background.preview',
		defaultMessage: 'Preview',
	},
	samplePanel: {
		id: 'app.settings.background.sample-panel',
		defaultMessage: 'Panels look like this',
	},
})
</script>

<template>
	<div class="flex flex-col gap-5">
		<div class="flex flex-col gap-2">
			<span class="font-semibold text-contrast">{{ formatMessage(messages.sourceLabel) }}</span>
			<Chips
				:model-value="selectedType"
				:items="sources"
				:format-label="formatSourceType"
				:aria-label="formatMessage(messages.sourceLabel)"
				:capitalize="false"
				@update:model-value="selectType"
			/>
		</div>

		<div v-if="selectedType === 'image'" class="flex flex-col gap-2">
			<div class="flex items-center gap-3">
				<Button :disabled="choosingImage" @click="chooseImage">
					<FolderOpenIcon aria-hidden="true" />
					{{ formatMessage(imagePath ? messages.changeImage : messages.chooseImage) }}
				</Button>
			</div>
			<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.imageHint) }}</p>
		</div>

		<div v-else-if="selectedType === 'color'" class="flex items-center gap-3">
			<label class="font-semibold text-contrast" for="background-color">
				{{ formatMessage(messages.color) }}
			</label>
			<input
				id="background-color"
				type="color"
				class="background-color-input"
				:value="colorValue"
				@input="setSource({ type: 'color', color: ($event.target as HTMLInputElement).value })"
			/>
		</div>

		<div v-else-if="selectedType === 'gradient'" class="flex flex-col gap-4">
			<div class="flex flex-wrap items-center gap-x-6 gap-y-3">
				<div class="flex items-center gap-3">
					<label class="font-semibold text-contrast" for="background-gradient-from">
						{{ formatMessage(messages.gradientFrom) }}
					</label>
					<input
						id="background-gradient-from"
						type="color"
						class="background-color-input"
						:value="(gradientValue ?? DEFAULT_GRADIENT).from"
						@input="updateGradient({ from: ($event.target as HTMLInputElement).value })"
					/>
				</div>
				<div class="flex items-center gap-3">
					<label class="font-semibold text-contrast" for="background-gradient-to">
						{{ formatMessage(messages.gradientTo) }}
					</label>
					<input
						id="background-gradient-to"
						type="color"
						class="background-color-input"
						:value="(gradientValue ?? DEFAULT_GRADIENT).to"
						@input="updateGradient({ to: ($event.target as HTMLInputElement).value })"
					/>
				</div>
			</div>
			<div class="flex flex-col gap-1">
				<span class="font-semibold text-contrast">{{ formatMessage(messages.gradientAngle) }}</span>
				<Slider
					:model-value="(gradientValue ?? DEFAULT_GRADIENT).angle"
					:min="0"
					:max="359"
					:step="1"
					@update:model-value="updateGradient({ angle: $event })"
				/>
			</div>
		</div>

		<template v-if="source.type !== 'none'">
			<div class="flex flex-col gap-1">
				<span class="font-semibold text-contrast">{{ formatMessage(messages.dim) }}</span>
				<span class="text-sm text-secondary">{{ formatMessage(messages.dimDescription) }}</span>
				<Slider
					:model-value="modelValue.dim"
					:min="0"
					:max="100"
					:step="1"
					@update:model-value="update({ dim: $event })"
				/>
			</div>

			<div class="flex flex-col gap-1">
				<span class="font-semibold text-contrast">{{ formatMessage(messages.blur) }}</span>
				<span class="text-sm text-secondary">
					{{
						formatMessage(
							blurDisabled ? messages.blurDisabledDescription : messages.blurDescription,
						)
					}}
				</span>
				<Slider
					:model-value="modelValue.blur"
					:min="0"
					:max="32"
					:step="1"
					:disabled="blurDisabled"
					@update:model-value="update({ blur: $event })"
				/>
			</div>

			<div class="flex flex-col gap-2">
				<span class="font-semibold text-contrast">{{ formatMessage(messages.preview) }}</span>
				<div class="background-preview">
					<AppBackground
						v-if="source.type !== 'none'"
						:config="modelValue"
						:image-url="imageUrl"
						:blur="blurDisabled ? 0 : modelValue.blur"
					/>
					<div
						class="background-preview-panel"
						:style="{ '--preview-opacity': `${panelOpacity}%` }"
					>
						{{ formatMessage(messages.samplePanel) }}
					</div>
				</div>
			</div>
		</template>
	</div>
</template>

<style scoped>
.background-color-input {
	width: 3.5rem;
	height: 2.25rem;
	padding: 0.25rem;
	cursor: pointer;
	border: 1px solid var(--surface-5);
	border-radius: var(--radius-md);
	background: var(--surface-4);
}

.background-preview {
	position: relative;
	display: flex;
	align-items: center;
	justify-content: center;
	height: 9rem;
	overflow: hidden;
	background-color: var(--surface-2);
	border: 1px solid var(--surface-5);
	border-radius: var(--radius-lg);
}

.background-preview-panel {
	position: relative;
	z-index: 1;
	padding: 0.75rem 1.25rem;
	font-weight: 600;
	color: var(--color-contrast);
	background-color: color-mix(in srgb, var(--surface-3-solid) var(--preview-opacity), transparent);
	border-radius: var(--radius-md);
}
</style>
