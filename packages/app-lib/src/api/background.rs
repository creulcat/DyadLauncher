//! Launcher background images: copying a user-chosen image into the app data dir and cleaning
//! up the ones nothing refers to any more.

use crate::state::instances::adapters::sqlite::instance_rows;
use crate::state::{Settings, State};
use crate::util::fetch::{sha1_async, write};
use crate::util::io;
use bytes::Bytes;
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageFormat, ImageReader};
use std::collections::HashSet;
use std::fs::File as StdFile;
use std::io::{BufReader, Cursor};
use std::path::Path;
use std::sync::{LazyLock, Mutex};

const BACKGROUNDS_DIR: &str = "backgrounds";
const BACKGROUND_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const BACKGROUND_MAX_DIMENSION: u32 = 2_560;
const BACKGROUND_MAX_SOURCE_DIMENSION: u32 = 12_000;
const BACKGROUND_MAX_DECODE_BYTES: u64 = 1024 * 1024 * 1024;
const BACKGROUND_JPEG_QUALITY: u8 = 90;

/// Files cached since startup that no setting refers to yet. A background is copied in as soon
/// as it is picked, but only saved when the user confirms, so [`collect_garbage`] must not
/// delete these out from under an unsaved draft. Empty after a restart, which is what cleans up
/// abandoned picks.
static PENDING: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

/// Copies `source` into the backgrounds cache, downscaled to at most
/// [`BACKGROUND_MAX_DIMENSION`] on the long side, and returns the path of the copy. The original
/// is never referenced afterwards, so moving or deleting it doesn't break the launcher.
pub async fn cache_image(source: &Path) -> crate::Result<String> {
    let state = State::get().await?;
    let source = source.to_path_buf();
    let (bytes, extension) =
        tokio::task::spawn_blocking(move || normalize_image(&source)).await??;

    let hash = sha1_async(bytes.clone()).await?;
    let file_name = format!("{hash}.{extension}");
    let path = state
        .directories
        .caches_dir()
        .join(BACKGROUNDS_DIR)
        .join(&file_name);
    write(&path, &bytes, &state.io_semaphore).await?;

    PENDING
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(file_name);

    Ok(io::canonicalize(path)?.to_string_lossy().to_string())
}

/// Deletes cached background files that neither the global settings nor any instance refers to.
/// Best effort: a failure is logged and never fails the edit that triggered it.
pub async fn collect_garbage() {
    if let Err(error) = collect_garbage_inner().await {
        tracing::warn!("Failed to clean up cached backgrounds: {error}");
    }
}

async fn collect_garbage_inner() -> crate::Result<()> {
    let state = State::get().await?;
    let dir = state.directories.caches_dir().join(BACKGROUNDS_DIR);
    if !dir.is_dir() {
        return Ok(());
    }

    let mut referenced = HashSet::new();
    let settings = Settings::get(&state.pool).await?;
    referenced.extend(file_name_of(settings.background.image_path()));
    for overrides in
        instance_rows::list_instance_launch_overrides(&state.pool).await?
    {
        let path = overrides
            .background
            .as_ref()
            .and_then(|background| background.image_path());
        referenced.extend(file_name_of(path));
    }

    let keep = files_to_keep(
        referenced,
        &mut PENDING.lock().unwrap_or_else(|e| e.into_inner()),
    );

    let mut entries = io::read_dir(&dir).await?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| io::IOError::with_path(e, &dir))?
    {
        let name = entry.file_name().to_string_lossy().to_string();
        if keep.contains(&name) {
            continue;
        }
        let is_file = entry
            .file_type()
            .await
            .map(|file_type| file_type.is_file())
            .unwrap_or(false);
        if is_file && let Err(error) = io::remove_file(entry.path()).await {
            tracing::warn!(
                "Failed to delete unused background {name}: {error}"
            );
        }
    }

    Ok(())
}

/// Files that must survive a cleanup: everything a setting refers to, plus files picked but not
/// yet saved. A pending file that has since been saved stops being pending, so it is deleted
/// normally once nothing refers to it any more.
fn files_to_keep(
    referenced: HashSet<String>,
    pending: &mut HashSet<String>,
) -> HashSet<String> {
    pending.retain(|name| !referenced.contains(name));
    referenced.union(pending).cloned().collect()
}

fn file_name_of(path: Option<&str>) -> Option<String> {
    Path::new(path?)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
}

fn normalize_image(source: &Path) -> crate::Result<(Bytes, &'static str)> {
    let file = StdFile::open(source).map_err(|error| {
        crate::ErrorKind::InputError(format!(
            "Could not open background image {}: {error}",
            source.display()
        ))
    })?;
    let length = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    if length > BACKGROUND_MAX_FILE_BYTES {
        return Err(crate::ErrorKind::InputError(format!(
            "Background image is too large (limit is {} MB)",
            BACKGROUND_MAX_FILE_BYTES / (1024 * 1024)
        ))
        .into());
    }

    let mut reader = ImageReader::new(BufReader::new(file))
        .with_guessed_format()
        .map_err(|error| {
            crate::ErrorKind::InputError(format!(
                "Could not identify background image format: {error}"
            ))
        })?;
    if !matches!(
        reader.format(),
        Some(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP)
    ) {
        return Err(crate::ErrorKind::InputError(
            "Backgrounds must be PNG, JPEG or WebP images".to_string(),
        )
        .into());
    }

    let mut limits = image::Limits::default();
    limits.max_image_width = Some(BACKGROUND_MAX_SOURCE_DIMENSION);
    limits.max_image_height = Some(BACKGROUND_MAX_SOURCE_DIMENSION);
    limits.max_alloc = Some(BACKGROUND_MAX_DECODE_BYTES);
    reader.limits(limits);

    let image = reader.decode().map_err(|error| {
        crate::ErrorKind::InputError(format!(
            "Could not decode background image: {error}"
        ))
    })?;
    let image = if image.width() > BACKGROUND_MAX_DIMENSION
        || image.height() > BACKGROUND_MAX_DIMENSION
    {
        image.resize(
            BACKGROUND_MAX_DIMENSION,
            BACKGROUND_MAX_DIMENSION,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        image
    };

    encode(image)
}

/// Opaque images become JPEG, which is far smaller for photos. Images that really use
/// transparency stay PNG.
fn encode(image: DynamicImage) -> crate::Result<(Bytes, &'static str)> {
    let encode_error = |error: image::ImageError| {
        crate::ErrorKind::InputError(format!(
            "Could not encode background image: {error}"
        ))
    };

    let rgba = image.to_rgba8();
    let uses_transparency = rgba.pixels().any(|pixel| pixel.0[3] < u8::MAX);
    let mut encoded = Cursor::new(Vec::new());

    if uses_transparency {
        DynamicImage::ImageRgba8(rgba)
            .write_to(&mut encoded, ImageFormat::Png)
            .map_err(encode_error)?;
        Ok((Bytes::from(encoded.into_inner()), "png"))
    } else {
        DynamicImage::ImageRgba8(rgba)
            .to_rgb8()
            .write_with_encoder(JpegEncoder::new_with_quality(
                &mut encoded,
                BACKGROUND_JPEG_QUALITY,
            ))
            .map_err(encode_error)?;
        Ok((Bytes::from(encoded.into_inner()), "jpg"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn write_test_image(
        dir: &Path,
        name: &str,
        width: u32,
        height: u32,
        alpha: u8,
        format: ImageFormat,
    ) -> std::path::PathBuf {
        let path = dir.join(name);
        RgbaImage::from_pixel(width, height, Rgba([200, 30, 90, alpha]))
            .save_with_format(&path, format)
            .unwrap();
        path
    }

    fn names(values: &[&str]) -> HashSet<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn cleanup_keeps_referenced_and_pending_files() {
        let mut pending = names(&["picked.jpg"]);
        let keep = files_to_keep(names(&["saved.png"]), &mut pending);

        assert_eq!(keep, names(&["saved.png", "picked.jpg"]));
        assert!(!keep.contains("orphan.jpg"));
    }

    #[test]
    fn saved_pending_files_stop_being_pending() {
        let mut pending = names(&["picked.jpg", "abandoned.jpg"]);

        let keep = files_to_keep(names(&["picked.jpg"]), &mut pending);
        assert_eq!(keep, names(&["picked.jpg", "abandoned.jpg"]));
        assert_eq!(pending, names(&["abandoned.jpg"]));

        let keep = files_to_keep(HashSet::new(), &mut pending);
        assert_eq!(keep, names(&["abandoned.jpg"]));
        assert!(!keep.contains("picked.jpg"));
    }

    #[test]
    fn file_names_are_compared_without_their_directory() {
        assert_eq!(
            file_name_of(Some("C:/data/caches/backgrounds/abc.jpg")),
            Some("abc.jpg".to_string())
        );
        assert_eq!(file_name_of(None), None);
    }

    #[test]
    fn large_opaque_images_are_downscaled_to_jpeg() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_test_image(
            dir.path(),
            "big.png",
            5_120,
            2_880,
            255,
            ImageFormat::Png,
        );

        let (bytes, extension) = normalize_image(&path).unwrap();
        assert_eq!(extension, "jpg");
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!(decoded.width(), BACKGROUND_MAX_DIMENSION);
        assert_eq!(decoded.height(), 1_440);
    }

    #[test]
    fn small_images_keep_their_size_and_transparency() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_test_image(
            dir.path(),
            "small.png",
            64,
            32,
            128,
            ImageFormat::Png,
        );

        let (bytes, extension) = normalize_image(&path).unwrap();
        assert_eq!(extension, "png");
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (64, 32));
    }

    #[test]
    fn unsupported_and_corrupt_files_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let gif = write_test_image(
            dir.path(),
            "anim.gif",
            8,
            8,
            255,
            ImageFormat::Gif,
        );
        assert!(normalize_image(&gif).is_err());

        let svg = dir.path().join("logo.svg");
        std::fs::write(&svg, "<svg xmlns='http://www.w3.org/2000/svg'/>")
            .unwrap();
        assert!(normalize_image(&svg).is_err());

        let junk = dir.path().join("junk.png");
        std::fs::write(&junk, b"not an image").unwrap();
        assert!(normalize_image(&junk).is_err());

        assert!(normalize_image(&dir.path().join("missing.png")).is_err());
    }
}
