const trimTrailingSlash = (url: string) => url.replace(/\/$/, '')

const labrinthBaseUrl = trimTrailingSlash(
	import.meta.env.MODRINTH_API_BASE_URL || 'https://api.modrinth.com',
)

export const config = {
	labrinthBaseUrl,
}
