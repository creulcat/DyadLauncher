<!-- Placeholder wordmark. Replace with your own branding. -->
<template>
	<svg
		xmlns="http://www.w3.org/2000/svg"
		viewBox="0 0 3307 593"
		:class="{ animate }"
	>
		<text x="620" y="420" font-family="sans-serif" font-size="360" font-weight="700" fill="currentColor">PROJECT</text>
		<g fill="var(--color-brand)">
			<circle class="ring ring--large" cx="295" cy="296" r="260" fill="none" stroke="var(--color-brand)" stroke-width="50" />
			<circle class="ring ring--small" cx="295" cy="296" r="150" fill="none" stroke="var(--color-brand)" stroke-width="50" />
			<path d="M180 296l70 70 130-160" fill="none" stroke="var(--color-brand)" stroke-width="50" stroke-linecap="round" stroke-linejoin="round" />
		</g>
	</svg>
</template>

<script setup>
defineProps({
	animate: {
		type: Boolean,
		default: false,
	},
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
