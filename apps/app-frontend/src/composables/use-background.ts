import { useQuery } from '@tanstack/vue-query'
import { computed, reactive, ref, watch, watchEffect } from 'vue'
import { useRoute } from 'vue-router'

import { getBackgroundImageUrl } from '@/helpers/background'
import type { BackgroundConfig } from '@/helpers/types'
import { instanceDetailQueryOptions } from '@/pages/instance/query-options'

import { useTheme } from './use-theme'

export const DEFAULT_BACKGROUND: BackgroundConfig = {
	source: { type: 'none' },
	dim: 40,
	blur: 8,
}

const globalBackground = ref<BackgroundConfig>({ ...DEFAULT_BACKGROUND })
const failedImageUrls = reactive(new Set<string>())

/**
 * A plain deep copy of a background config. Configs are pure JSON, and this copes with the Vue
 * reactive proxies they are usually wrapped in, which `structuredClone` rejects.
 */
export function cloneBackground(config: BackgroundConfig): BackgroundConfig {
	return JSON.parse(JSON.stringify(config))
}

function canonicalJson(value: unknown): string {
	return JSON.stringify(value ?? null, (_key, entry) =>
		entry && typeof entry === 'object' && !Array.isArray(entry)
			? Object.fromEntries(Object.entries(entry).sort(([a], [b]) => a.localeCompare(b)))
			: entry,
	)
}

/** Whether two backgrounds are equal, ignoring key order. `null` (inherit) only equals `null`. */
export function sameBackground(
	a: BackgroundConfig | null | undefined,
	b: BackgroundConfig | null | undefined,
): boolean {
	return canonicalJson(a) === canonicalJson(b)
}

/** Sets the launcher-wide background, which every page without an instance override shows. */
export function setGlobalBackground(config: BackgroundConfig): void {
	globalBackground.value = config
}

const instancePreview = ref<{ instanceId: string; background: BackgroundConfig | null } | null>(
	null,
)

/**
 * Shows an instance's unsaved background on its own pages, as if it were already saved. A preview
 * whose `background` is `null` previews inheriting the launcher background; pass `null` instead of
 * a preview to stop previewing.
 */
export function setInstanceBackgroundPreview(
	preview: { instanceId: string; background: BackgroundConfig | null } | null,
): void {
	instancePreview.value = preview
}

export function useGlobalBackground() {
	return globalBackground
}

/**
 * The background the current page should show: the open instance's own override on `/instance/…`
 * pages (when it has one), otherwise the global background. A missing or unreadable image counts
 * as no background instead of an error.
 */
export function useEffectiveBackground() {
	const route = useRoute()
	const theme = useTheme()

	const instanceId = computed(() =>
		route.path.startsWith('/instance/') ? String(route.params.id ?? '') : '',
	)
	const instanceQuery = useQuery(
		computed(() => ({
			...instanceDetailQueryOptions(instanceId.value),
			enabled: !!instanceId.value,
		})),
	)

	const config = computed<BackgroundConfig>(() => {
		if (!instanceId.value) return globalBackground.value

		const preview = instancePreview.value
		const override =
			preview?.instanceId === instanceId.value
				? preview.background
				: instanceQuery.data.value?.background
		return override ?? globalBackground.value
	})

	const imageUrl = computed(() => {
		const { source } = config.value
		return source.type === 'image' ? getBackgroundImageUrl(source.path) : null
	})

	watch(
		imageUrl,
		(url) => {
			if (!url || failedImageUrls.has(url)) return
			const probe = new Image()
			probe.onerror = () => failedImageUrls.add(url)
			probe.src = url
		},
		{ immediate: true },
	)

	const active = computed(() => {
		const { source } = config.value
		if (source.type === 'none') return false
		return !(imageUrl.value && failedImageUrls.has(imageUrl.value))
	})

	/** Blur is skipped entirely when Advanced rendering is off, leaving a plain dim. */
	const blur = computed(() => (theme.advancedRendering ? config.value.blur : 0))

	watchEffect(() => {
		const root = document.documentElement
		root.classList.toggle('has-background', active.value)
		root.style.setProperty('--background-dim', String(config.value.dim))
	})

	return { config, active, imageUrl, blur }
}
