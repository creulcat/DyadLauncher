import { createContext } from '@modrinth/ui'
import type { ComputedRef, Ref } from 'vue'

import type { GameInstance } from '@/helpers/types'
import type { UnsavedChangesController } from '@/providers/app-settings-modal'

export interface InstanceSettingsContext {
	instance: ComputedRef<GameInstance>
	offline?: boolean
	isMinecraftServer: Ref<boolean>
	onUnlinked: () => void
	closeModal?: () => void
	/** Lets a tab put changes it has not saved yet behind the modal's Save / Reset bar. */
	registerUnsavedChangesController: (controller: UnsavedChangesController | null) => void
}

export const [injectInstanceSettings, provideInstanceSettings] =
	createContext<InstanceSettingsContext>('InstanceSettingsModal', 'instanceSettings')
