import { provideUserCountry } from '@modrinth/ui'
import { ref } from 'vue'

// The country only decides whether Imgur images in rendered project descriptions are proxied, and
// looking it up means a GeoIP request to Modrinth at every launch. A fixed value keeps the launcher
// from making that request; the cost is that Imgur is never proxied for users in blocked regions.
export function setupUserCountryProvider() {
	return provideUserCountry(ref('US'))
}
