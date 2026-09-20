<script setup lang="ts">
import { DiscordIcon, ImportIcon, ModrinthIcon, PlusIcon } from '@modrinth/assets'
import {
	Button,
	defineMessages,
	injectNotificationManager,
	IntlFormatted,
	Toggle,
	useVIntl,
} from '@modrinth/ui'
import { inject, onMounted, onUnmounted, ref } from 'vue'

import { detectModrinthAppInstall } from '@/helpers/migrate-modrinth-app'
import { get as getSettings, set as setSettings } from '@/helpers/settings.ts'

import dyadMark from '../../assets/welcome/dyad-mark.svg?url'

const showCreationModal = inject<() => void>('showCreationModal')
const showImportModal = inject<() => void>('showImportModal')
// Provided from `App.vue` rather than owned here - this component unmounts
// (via its parent's `v-if="!hasCreatedInstance"`) the moment an import
// creates its first instance, which would otherwise tear a locally-owned
// modal down mid-import.
const showMigrateModrinthAppModal = inject<() => void>('showMigrateModrinthAppModal')

const modrinthAppDetected = ref(false)

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()

// Opt-in (off by default). This reflects the real setting rather than a one-time answer, since
// this screen shows again whenever there are no instances, and it must not re-ask anyone
const discordRichPresence = ref(false)
const savingDiscordRichPresence = ref(false)

async function setDiscordRichPresence(enabled: boolean) {
	if (savingDiscordRichPresence.value) return

	const previous = discordRichPresence.value
	savingDiscordRichPresence.value = true
	discordRichPresence.value = enabled
	try {
		// Read the settings right before writing, since the whole object is saved at once
		const settings = await getSettings()
		await setSettings({ ...settings, discord_rpc: enabled })
	} catch (error) {
		discordRichPresence.value = previous
		handleError(error)
	} finally {
		savingDiscordRichPresence.value = false
	}
}

const messages = defineMessages({
	welcomeTitle: {
		id: 'app.welcome-screen.title',
		defaultMessage: 'Welcome to Dyad',
	},
	welcomeDescription: {
		id: 'app.welcome-screen.description',
		defaultMessage: 'Ready to start playing?',
	},
	createInstance: {
		id: 'app.welcome-screen.create-instance',
		defaultMessage: 'Create an instance',
	},
	quickCreateHint: {
		id: 'app.welcome-screen.quick-create-hint',
		defaultMessage: 'Press <shortcut>N</shortcut> to quick create an instance',
	},
	discordRichPresenceTitle: {
		id: 'app.welcome-screen.discord-rich-presence.title',
		defaultMessage: 'Show on Discord',
	},
	discordRichPresenceDescription: {
		id: 'app.welcome-screen.discord-rich-presence.description',
		defaultMessage:
			'Let friends see what you are playing with Discord Rich Presence. Off by default; change it any time in Settings, under Behavior.',
	},
	importPrompt: {
		id: 'app.welcome-screen.import-prompt',
		defaultMessage: 'Escaping another launcher?',
	},
	importFromLauncher: {
		id: 'app.welcome-screen.import-from-launcher',
		defaultMessage: 'Import from launcher',
	},
	importFromModrinthApp: {
		id: 'app.welcome-screen.import-from-modrinth-app',
		defaultMessage: 'Import from Modrinth App',
	},
})

const offline = ref(!navigator.onLine)

function handleOffline() {
	offline.value = true
}

function handleOnline() {
	offline.value = false
}

function handleQuickCreate(event: KeyboardEvent) {
	const target = event.target as HTMLElement | null
	if (
		event.key.toLowerCase() !== 'n' ||
		event.repeat ||
		event.metaKey ||
		event.ctrlKey ||
		event.altKey ||
		target?.isContentEditable ||
		['INPUT', 'TEXTAREA', 'SELECT'].includes(target?.tagName ?? '')
	) {
		return
	}

	if (!offline.value) {
		event.preventDefault()
		showCreationModal?.()
	}
}

onMounted(() => {
	window.addEventListener('offline', handleOffline)
	window.addEventListener('online', handleOnline)
	window.addEventListener('keydown', handleQuickCreate)

	detectModrinthAppInstall()
		.then((detected) => {
			modrinthAppDetected.value = !!detected
		})
		.catch(() => {
			modrinthAppDetected.value = false
		})

	getSettings()
		.then((settings) => {
			discordRichPresence.value = settings.discord_rpc
		})
		.catch(() => {
			discordRichPresence.value = false
		})
})

onUnmounted(() => {
	window.removeEventListener('offline', handleOffline)
	window.removeEventListener('online', handleOnline)
	window.removeEventListener('keydown', handleQuickCreate)
})
</script>

<template>
	<div class="flex flex-col min-h-full px-6 pb-6 pt-16">
		<div class="relative flex grow items-center justify-center">
			<div class="relative isolate flex flex-col items-center gap-6">
				<div
					class="dot-pattern pointer-events-none absolute left-1/2 -top-52 -z-10 h-[29.875rem] w-[min(25.9375rem,80vw)] -translate-x-1/2 rounded-2xl [@media(max-height:700px)]:h-[23rem]"
					aria-hidden="true"
				/>
				<div class="size-[6.25rem]">
					<img :src="dyadMark" alt="" class="pointer-events-none size-full" />
				</div>
				<div class="flex flex-col items-center gap-2">
					<h1 class="m-0 flex items-center gap-2 text-2xl font-semibold leading-8 text-contrast">
						{{ formatMessage(messages.welcomeTitle) }}
					</h1>
					<p class="m-0 text-center text-base leading-6 text-primary">
						{{ formatMessage(messages.welcomeDescription) }}
					</p>
				</div>
				<div class="flex w-72 flex-col items-center gap-4">
					<Button
						type="colored"
						color="brand"
						size="lg"
						class="!shadow-none"
						:disabled="offline"
						@click="showCreationModal?.()"
					>
						<PlusIcon />
						{{ formatMessage(messages.createInstance) }}
					</Button>
					<span class="flex items-center gap-1 text-sm leading-5 text-secondary">
						<IntlFormatted :message-id="messages.quickCreateHint">
							<template #shortcut="{ children }">
								<kbd
									class="inline-flex h-5 min-w-5 items-center justify-center rounded-md border border-solid border-surface-5 bg-button-bg px-1 text-xs font-normal leading-4 text-primary"
								>
									<component :is="() => children" />
								</kbd>
							</template>
						</IntlFormatted>
					</span>
				</div>
				<div
					class="flex w-80 max-w-full items-center justify-between gap-4 rounded-2xl border border-solid border-surface-5 bg-button-bg p-4"
				>
					<div class="flex min-w-0 flex-col gap-1">
						<span
							id="welcome-discord-rich-presence-label"
							class="flex items-center gap-2 text-base font-semibold leading-6 text-contrast"
						>
							<DiscordIcon class="size-5 shrink-0" aria-hidden="true" />
							{{ formatMessage(messages.discordRichPresenceTitle) }}
						</span>
						<span class="text-sm leading-5 text-secondary">
							{{ formatMessage(messages.discordRichPresenceDescription) }}
						</span>
					</div>
					<Toggle
						id="welcome-discord-rich-presence"
						aria-labelledby="welcome-discord-rich-presence-label"
						:model-value="discordRichPresence"
						:disabled="savingDiscordRichPresence"
						@update:model-value="setDiscordRichPresence"
					/>
				</div>
			</div>
		</div>
		<div
			class="flex flex-col h-max items-center justify-end gap-4 text-sm leading-5 text-secondary"
		>
			<span class="whitespace-nowrap">{{ formatMessage(messages.importPrompt) }}</span>
			<div class="flex items-center gap-2">
				<Button size="lg" class="!font-medium" :disabled="offline" @click="showImportModal?.()">
					<ImportIcon />
					{{ formatMessage(messages.importFromLauncher) }}
				</Button>
				<Button
					v-if="modrinthAppDetected"
					size="lg"
					class="!font-medium"
					:disabled="offline"
					@click="showMigrateModrinthAppModal?.()"
				>
					<ModrinthIcon />
					{{ formatMessage(messages.importFromModrinthApp) }}
				</Button>
			</div>
		</div>
	</div>
</template>

<style scoped>
.dot-pattern {
	background-image: radial-gradient(
		circle,
		color-mix(in srgb, var(--color-text-primary) 25%, transparent) 0.5px,
		transparent 0.75px
	);
	background-size: 0.5625rem 0.5625rem;
	opacity: 0.8;
	-webkit-mask-image: radial-gradient(ellipse at center, black 10%, transparent 68%);
	mask-image: radial-gradient(ellipse at center, black 10%, transparent 68%);
	-webkit-mask-repeat: no-repeat;
	mask-repeat: no-repeat;
}
</style>
