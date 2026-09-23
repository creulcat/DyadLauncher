import { convertFileSrc, invoke } from '@tauri-apps/api/core'

/**
 * Copies a user-chosen image into the launcher's backgrounds cache (downscaled, re-encoded) and
 * returns the path of the copy, to be stored in a `BackgroundConfig` of type `image`.
 */
export async function cacheBackgroundImage(path: string): Promise<string> {
	return await invoke('plugin:background|background_cache_image', { path })
}

export function getBackgroundImageUrl(path: string): string {
	return convertFileSrc(path)
}
