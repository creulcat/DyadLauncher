<script setup lang="ts">
import { BanIcon, CopyIcon, LinkIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'

import type { SymlinkAction } from '@/helpers/migrate-modrinth-app'

defineProps<{
	modelValue: SymlinkAction
}>()

const emit = defineEmits<{
	'update:modelValue': [action: SymlinkAction]
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
	copy: {
		id: 'app.migrate-modrinth-app.symlink.copy',
		defaultMessage: 'Copy',
	},
	recreate: {
		id: 'app.migrate-modrinth-app.symlink.recreate',
		defaultMessage: 'Link',
	},
	ignore: {
		id: 'app.migrate-modrinth-app.symlink.ignore',
		defaultMessage: 'Skip',
	},
})

const options: { action: SymlinkAction; icon: typeof CopyIcon; label: typeof messages.copy }[] = [
	{ action: 'copy', icon: CopyIcon, label: messages.copy },
	{ action: 'recreate', icon: LinkIcon, label: messages.recreate },
	{ action: 'ignore', icon: BanIcon, label: messages.ignore },
]
</script>

<template>
	<div class="flex items-center gap-1">
		<Button
			v-for="option in options"
			:key="option.action"
			size="xs"
			:type="modelValue === option.action ? 'colored' : 'outlined'"
			:color="modelValue === option.action ? 'brand' : undefined"
			@click="emit('update:modelValue', option.action)"
		>
			<component :is="option.icon" class="size-3.5" />
			{{ formatMessage(option.label) }}
		</Button>
	</div>
</template>
