<!-- Placeholder wordmark. Replace with your own branding. -->
<template>
	<svg
		xmlns="http://www.w3.org/2000/svg"
		viewBox="0 0 3307 593"
		:class="{ animate: loading }"
	>
		<text x="620" y="420" font-family="sans-serif" font-size="360" font-weight="700" fill="currentColor">
			{{ label }}
		</text>
		<g fill="var(--color-brand)">
			<circle class="ring ring--large" cx="295" cy="296" r="260" fill="none" stroke="var(--color-brand)" stroke-width="50" />
			<circle class="ring ring--small" cx="295" cy="296" r="150" fill="none" stroke="var(--color-brand)" stroke-width="50" />
			<path d="M180 296l70 70 130-160" fill="none" stroke="var(--color-brand)" stroke-width="50" stroke-linecap="round" stroke-linejoin="round" />
		</g>
	</svg>
</template>

<script setup lang="ts">
const loading = useLoading()

const config = useRuntimeConfig()
const flags = useFeatureFlags()

// Kept as a lightweight visual signal of which backend the app is talking to
// (production / staging / local / a non-default API), independent of branding.
const api = computed(() => {
	if (flags.value.demoMode) return 'prod'

	const apiUrl = config.public.apiBaseUrl
	if (apiUrl.startsWith('https://api.modrinth.com')) {
		return 'prod'
	} else if (apiUrl.startsWith('https://staging-api.modrinth.com')) {
		return 'staging'
	} else if (apiUrl.startsWith('localhost') || apiUrl.startsWith('127.0.0.1')) {
		return 'localhost'
	}
	return 'foreign'
})

const label = computed(() => {
	switch (api.value) {
		case 'staging':
			return 'PROJECT (staging)'
		case 'localhost':
			return 'PROJECT (local)'
		case 'foreign':
			return 'PROJECT (custom)'
		default:
			return 'PROJECT'
	}
})
</script>

<style lang="scss" scoped>
.animate {
	.ring {
		transform-origin: center;
		transform-box: fill-box;
		animation-fill-mode: forwards;
		transition: transform 2s ease-in-out;
		&--large {
			animation: spin 1s ease-in-out infinite forwards;
		}
		&--small {
			animation: spin 2s ease-in-out infinite reverse;
		}
	}
	@keyframes spin {
		from {
			transform: rotate(0deg);
		}
		to {
			transform: rotate(360deg);
		}
	}
}
</style>
