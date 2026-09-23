import { provide, useTemplateRef } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'

import type MigrateModrinthAppModal from '@/components/ui/modal/MigrateModrinthAppModal.vue'

/**
 * Hosts the goal 6 import modal at the app root (see `App.vue`) rather than
 * inside `WelcomeScreen`, which unmounts itself (via its parent's
 * `v-if="!hasCreatedInstance"`) the moment the import creates its first
 * instance - previously that tore the modal down mid-import, cutting off
 * progress for every instance after the first. Mirrors
 * `setupCreationModal`'s `installationModal`/`showCreationModal` pattern.
 */
export function setupMigrateModrinthAppModalProvider() {
	const migrateModrinthAppModal =
		useTemplateRef<ComponentExposed<typeof MigrateModrinthAppModal>>('migrateModrinthAppModal')

	provide('showMigrateModrinthAppModal', () => {
		migrateModrinthAppModal.value?.show()
	})

	return { migrateModrinthAppModal }
}
