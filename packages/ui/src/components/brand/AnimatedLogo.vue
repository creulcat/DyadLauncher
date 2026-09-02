<!-- Placeholder animated logo mark. Replace with your own branding. -->
<template>
	<div>
		<svg class="rotate outer" width="100%" height="100%" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
			<circle cx="50" cy="50" r="46" fill="none" stroke="var(--color-brand)" stroke-width="6" stroke-dasharray="180 100" />
		</svg>
		<svg class="rotate inner" width="100%" height="100%" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
			<circle cx="50" cy="50" r="32" fill="none" stroke="var(--color-brand)" stroke-width="6" stroke-dasharray="120 80" />
		</svg>
		<svg width="100%" height="100%" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
			<path d="M35 52l12 12 20-26" fill="none" stroke="var(--color-brand)" stroke-width="8" stroke-linecap="round" stroke-linejoin="round" />
		</svg>
	</div>
</template>

<style lang="scss" scoped>
div {
	height: 5rem;

	svg {
		width: 5rem;
		height: 5rem;
		position: absolute;
		&.rotate {
			animation: rotate 4s infinite linear;
			&.inner {
				animation: rotate 6s infinite linear reverse;
			}
		}
		@keyframes rotate {
			0% {
				transform: rotate(0);
			}
			100% {
				transform: rotate(360deg);
			}
		}
	}
}
</style>
