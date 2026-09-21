import { computed, ref, shallowRef } from 'vue'

import type { UnsavedChangesController } from '@/providers/app-settings-modal'

const NO_STATE: Record<string, unknown> = {}

/**
 * State for the "You have unsaved changes" bar of a tabbed modal. A settings tab registers a
 * controller, and the modal passes `shown` to `floating-action-bar-shown`, `canLeave` to
 * `before-hide` / `before-tab-change`, and the rest to `UnsavedChangesPopup`.
 */
export function useUnsavedChangesBar() {
	const popup = ref<{ nudge: () => void } | null>(null)
	const controller = shallowRef<UnsavedChangesController | null>(null)

	const original = computed(() => controller.value?.getOriginal() ?? NO_STATE)
	const modified = computed(() => controller.value?.getModified() ?? NO_STATE)
	const saving = computed(() => controller.value?.isSaving() ?? false)
	const shown = computed(() => (controller.value?.hasChanges() ?? false) || saving.value)

	function register(next: UnsavedChangesController | null): void {
		controller.value = next
	}

	/** Refuses to leave while there are unsaved changes, drawing attention to the bar instead. */
	function canLeave(): boolean {
		if (!shown.value) return true
		popup.value?.nudge()
		return false
	}

	function reset(): void {
		controller.value?.reset()
	}

	function save(): void {
		void controller.value?.save()
	}

	return { popup, original, modified, saving, shown, register, canLeave, reset, save }
}
