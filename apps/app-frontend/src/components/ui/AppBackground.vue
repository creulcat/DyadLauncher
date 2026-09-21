<script setup lang="ts">
import { computed } from 'vue'

import type { BackgroundConfig } from '@/helpers/types'

const props = defineProps<{
	config: BackgroundConfig
	imageUrl: string | null
	blur: number
}>()

const sourceStyle = computed(() => {
	const { source } = props.config
	const style: Record<string, string> = {}

	switch (source.type) {
		case 'color':
			style.backgroundColor = source.color
			break
		case 'gradient':
			style.backgroundImage = `linear-gradient(${source.angle}deg, ${source.from}, ${source.to})`
			break
		case 'image':
			if (props.imageUrl) style.backgroundImage = `url("${props.imageUrl}")`
			break
	}

	if (props.blur > 0) {
		// Blurring samples past the layer's edge, so it is oversized and clipped by the parent
		style.inset = `${-props.blur * 2}px`
		style.filter = `blur(${props.blur}px)`
	}

	return style
})
</script>

<template>
	<div class="app-background" aria-hidden="true">
		<div class="app-background-source" :style="sourceStyle"></div>
		<div class="app-background-dim" :style="{ opacity: config.dim / 100 }"></div>
	</div>
</template>

<style scoped>
.app-background,
.app-background-source,
.app-background-dim {
	position: absolute;
	inset: 0;
	pointer-events: none;
}

.app-background {
	z-index: 0;
	overflow: hidden;
}

.app-background-source {
	background-position: center;
	background-size: cover;
	background-repeat: no-repeat;
}

.app-background-dim {
	background-color: var(--surface-1-solid);
}
</style>
