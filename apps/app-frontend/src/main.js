import 'floating-vue/dist/style.css'
import 'overlayscrollbars/overlayscrollbars.css'
import '@/assets/stylesheets/background.scss'

import { VueQueryPlugin } from '@tanstack/vue-query'
import FloatingVue from 'floating-vue'
import { createApp } from 'vue'

import App from '@/App.vue'
import { overlayScrollbarsDirective } from '@/directives/overlayScrollbars'
import i18nPlugin from '@/plugins/i18n'
import i18nDebugPlugin from '@/plugins/i18n-debug'
import router from '@/routes'

let app = createApp(App)

app.use(VueQueryPlugin)
app.use(router)
app.use(FloatingVue, {
	themes: {
		'ribbit-popout': {
			$extend: 'dropdown',
			placement: 'bottom-end',
			instantMove: true,
			distance: 8,
		},
		'dismissable-prompt': {
			$extend: 'dropdown',
			placement: 'bottom-start',
		},
	},
})
app.use(i18nPlugin)
app.use(i18nDebugPlugin)
app.directive('overlay-scrollbars', overlayScrollbarsDirective)

app.mount('#app')
