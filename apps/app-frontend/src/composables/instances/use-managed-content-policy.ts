import { type ContentActionWarning, type ContentItem, defineMessages, useVIntl } from '@modrinth/ui'
import { computed, type Ref } from 'vue'

import type { GameInstance } from '@/helpers/types'

export function useManagedContentPolicy(instance: Ref<GameInstance>) {
	const { formatMessage } = useVIntl()
	const isQuarantined = computed(() => instance.value.quarantined)

	function isManagedContent(_item: ContentItem) {
		return isQuarantined.value
	}

	function canMutateContent(item: ContentItem) {
		return !isManagedContent(item)
	}

	function canUpdateContent(item: ContentItem) {
		return (
			canMutateContent(item) && !!item.file_path && !!item.has_update && !!item.update_version_id
		)
	}

	function deleteWarning(items: ContentItem[]): ContentActionWarning | null {
		if (!items.some(isManagedContent)) return null
		return {
			admonitionHeader: formatMessage(messages.warningHeader),
			admonitionBody: formatMessage(
				items.length === 1 ? messages.deleteSingleBody : messages.deleteBulkBody,
			),
			actionLabel: formatMessage(
				items.length === 1 ? messages.deleteButton : messages.deleteManyButton,
				{ count: items.length },
			),
		}
	}

	function disableWarning(items: ContentItem[]): ContentActionWarning | null {
		if (!items.some(isManagedContent)) return null
		return {
			admonitionHeader: formatMessage(messages.warningHeader),
			admonitionBody: formatMessage(
				items.length === 1 ? messages.disableSingleBody : messages.disableBulkBody,
			),
			actionLabel: formatMessage(
				items.length === 1 ? messages.disableButton : messages.disableManyButton,
				{ count: items.length },
			),
		}
	}

	return {
		isQuarantined,
		isManagedContent,
		canMutateContent,
		canUpdateContent,
		deleteWarning,
		disableWarning,
	}
}

const messages = defineMessages({
	warningHeader: {
		id: 'content.shared-instance.warning-header',
		defaultMessage: 'This is part of the shared instance',
	},
	deleteSingleBody: {
		id: 'content.shared-instance.delete-single-body',
		defaultMessage:
			'Deleting it only changes your local copy. Future shared instance updates may restore or change it again.',
	},
	deleteBulkBody: {
		id: 'content.shared-instance.delete-bulk-body',
		defaultMessage:
			'Some selected projects are part of the shared instance. Deleting them only changes your local copy, and future shared instance updates may restore or change them again.',
	},
	deleteButton: { id: 'content.shared-instance.delete-button', defaultMessage: 'Delete anyway' },
	deleteManyButton: {
		id: 'content.shared-instance.delete-many-button',
		defaultMessage: 'Delete {count, number} projects anyway',
	},
	disableSingleBody: {
		id: 'content.shared-instance.disable-single-body',
		defaultMessage:
			'Disabling it only changes your local copy. Future shared instance updates may re-enable, restore, or change it again.',
	},
	disableBulkBody: {
		id: 'content.shared-instance.disable-bulk-body',
		defaultMessage:
			'Some selected projects are part of the shared instance. Disabling them only changes your local copy, and future shared instance updates may re-enable, restore, or change them again.',
	},
	disableButton: { id: 'content.shared-instance.disable-button', defaultMessage: 'Disable anyway' },
	disableManyButton: {
		id: 'content.shared-instance.disable-many-button',
		defaultMessage: 'Disable {count, number} projects anyway',
	},
})
