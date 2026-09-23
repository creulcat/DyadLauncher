import type { AbstractPopupNotificationManager, AbstractWebNotificationManager } from '@modrinth/ui'

import type { InstanceIconConfig } from '@/helpers/types'

import type { AppEvents } from './app-events'
import { setupCreationModal } from './setup/creation-modal'
import { setupFileDropProvider } from './setup/file-drop'
import { setupFilePickerProvider } from './setup/file-picker'
import { setupImageViewerEditorProvider } from './setup/image-viewer-editor'
import { setupInstanceImportProvider } from './setup/instance-import'
import { setupMigrateModrinthAppModalProvider } from './setup/migrate-modrinth-app-modal'
import { setupTagsProvider } from './setup/tags'
import { setupUserCountryProvider } from './setup/user-country'

export function setupProviders(
	notificationManager: AbstractWebNotificationManager,
	_popupNotificationManager: AbstractPopupNotificationManager,
	_appEvents: AppEvents,
	getGeneratedIconConfig?: (iconPath: string) => InstanceIconConfig | null,
) {
	setupUserCountryProvider()
	setupTagsProvider(notificationManager)
	setupFileDropProvider()
	setupFilePickerProvider()
	setupImageViewerEditorProvider()
	setupInstanceImportProvider(notificationManager)

	return {
		...setupCreationModal(notificationManager, getGeneratedIconConfig),
		...setupMigrateModrinthAppModalProvider(),
	}
}
