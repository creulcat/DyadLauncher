<script setup lang="ts">
import { ImportIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { ref } from 'vue'

import MigrateModrinthAppModal from '@/components/ui/modal/MigrateModrinthAppModal.vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	title: {
		id: 'app.settings.migrate-modrinth-app.title',
		defaultMessage: 'Import from Modrinth App',
	},
	description: {
		id: 'app.settings.migrate-modrinth-app.description',
		defaultMessage:
			'Bring instances, content, and settings over from an existing official Modrinth App install.',
	},
	button: {
		id: 'app.settings.migrate-modrinth-app.button',
		defaultMessage: 'Import...',
	},
})

const migrateModal = ref<InstanceType<typeof MigrateModrinthAppModal> | null>(null)
</script>

<template>
	<div class="flex items-center justify-between gap-4">
		<div>
			<h2 class="m-0 text-lg font-semibold text-contrast">
				{{ formatMessage(messages.title) }}
			</h2>
			<p class="m-0 mt-1">
				{{ formatMessage(messages.description) }}
			</p>
		</div>
		<Button type="colored" color="brand" @click="migrateModal?.show()">
			<ImportIcon />
			{{ formatMessage(messages.button) }}
		</Button>
	</div>
	<MigrateModrinthAppModal ref="migrateModal" />
</template>
