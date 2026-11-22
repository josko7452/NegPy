//! One frame through the export path: layout, pixels, container and disk.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::domain::models::{
    ExportConfig, ExportPreset, ExportPresetOutputMode, FinishConfig, ToningConfig, WorkspaceConfig,
};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;
use crate::kernel::system::paths::replace_file;
use crate::services::export::encode::encode_export;
use crate::services::export::metadata::export_embed_plan;
use crate::services::export::pixels::{export_pixels, target_profile};
use crate::services::export::print::{apply_layout, effective_border_color};

/// The destination folder for one source, by the preset's output mode.
#[must_use]
pub fn resolve_output_dir(source_path: &Path, preset: &ExportPreset) -> PathBuf {
    let source_dir = source_path
        .parent()
        .map_or_else(PathBuf::new, Path::to_path_buf);
    match preset.output_mode {
        ExportPresetOutputMode::SubfolderOfSource => {
            if preset.output_subfolder.is_empty() {
                source_dir
            } else {
                source_dir.join(&preset.output_subfolder)
            }
        }
        ExportPresetOutputMode::Absolute => {
            let absolute = preset.output_path.trim();
            if absolute.is_empty() {
                source_dir
            } else {
                PathBuf::from(absolute)
            }
        }
        ExportPresetOutputMode::SameAsSource => source_dir,
    }
}

/// The destination folder for a raw export config, which carries only its path.
#[must_use]
pub fn resolve_export_dir(source_path: &Path, config: &ExportConfig) -> PathBuf {
    if !config.export_path.trim().is_empty() {
        return PathBuf::from(config.export_path.trim());
    }
    source_path
        .parent()
        .map_or_else(PathBuf::new, Path::to_path_buf)
}

/// One rendered export ready for its file.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderedExport {
    pub pixels: crate::services::export::pixels::ExportPixels,
    pub extension: &'static str,
}

/// Lay out, convert and encode one rendered float buffer.
pub fn render_export(
    buffer: &ImageBuffer,
    config: &ExportConfig,
    color_space: &str,
    finish: &FinishConfig,
    toning: &ToningConfig,
    user_icc_dir: &Path,
) -> Result<RenderedExport, String> {
    let border_color = effective_border_color(finish, toning);
    let layout = apply_layout(buffer, config, finish.border_size, &border_color, finish);
    let target = target_profile(
        color_space,
        config.icc_output_path.as_deref().map(Path::new),
        user_icc_dir,
    )
    .0;
    let pixels = export_pixels(
        &layout.image.pixels,
        layout.image.width,
        layout.image.height,
        config,
        &target,
        config.icc_input_path.as_deref().map(Path::new),
        config.icc_output_path.as_deref().map(Path::new),
        user_icc_dir,
    );
    let encoded = encode_export(&pixels, config)?;
    Ok(RenderedExport {
        pixels,
        extension: encoded.extension,
    })
}

/// Encode and write one rendered export beside its source, replacing the destination.
pub fn write_export(
    buffer: &ImageBuffer,
    source_path: &Path,
    config: &ExportConfig,
    workspace: &WorkspaceConfig,
    color_space: &str,
    user_icc_dir: &Path,
) -> Result<PathBuf, String> {
    write_export_with_names(
        buffer,
        source_path,
        config,
        workspace,
        color_space,
        user_icc_dir,
        &mut HashSet::new(),
    )
}

/// The same write, with the batch's names shared so two frames never take one path.
pub fn write_export_with_names(
    buffer: &ImageBuffer,
    source_path: &Path,
    config: &ExportConfig,
    workspace: &WorkspaceConfig,
    color_space: &str,
    user_icc_dir: &Path,
    written: &mut HashSet<PathBuf>,
) -> Result<PathBuf, String> {
    let rendered = render_export(
        buffer,
        config,
        color_space,
        &workspace.finish,
        &workspace.toning,
        user_icc_dir,
    )?;
    let stem = crate::services::export::templating::render_export_filename(
        &source_path.display().to_string(),
        config,
        workspace.finish.border_size,
        0,
        Some(&workspace.metadata),
        "",
    );
    let directory = resolve_export_dir(source_path, config);
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("Could not create {}: {error}", directory.display()))?;
    let path = unique_path(
        &directory,
        &stem,
        rendered.extension,
        config.overwrite,
        written,
    );
    written.insert(path.clone());
    let encoded = encode_export(&rendered.pixels, config)?;
    let resolution = Some(crate::features::metadata::resolution::Resolution::from_dpi(
        crate::services::export::print::effective_dpi(config) as f64,
    ));
    let source_exif = crate::infrastructure::loaders::exif_block::read_exif_file(source_path);
    let source_exif = source_exif.map(|raw| crate::features::metadata::exif_read::SourceExif {
        zeroth: raw.zeroth,
        exif: raw.exif,
        gps: raw.gps,
    });
    let bytes = match export_embed_plan(
        &workspace.metadata,
        source_exif.as_ref(),
        source_path,
        None,
        resolution.as_ref(),
    ) {
        Some(plan) => plan.embed(encoded.bytes, config.export_fmt),
        None => encoded.bytes,
    };
    let temporary = directory.join(format!(".{stem}.{}.part", std::process::id()));
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("Could not write {}: {error}", temporary.display()))?;
    replace_file(&temporary, &path)
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    Ok(path)
}

/// The file name to write: `stem.ext`, or `stem_2.ext` and up when an earlier file
/// exists and the config does not allow overwriting.
#[must_use]
pub fn unique_path(
    directory: &Path,
    stem: &str,
    extension: &str,
    overwrite: bool,
    written: &mut HashSet<PathBuf>,
) -> PathBuf {
    let first = directory.join(format!("{stem}.{extension}"));
    if !written.contains(&first) && (overwrite || !first.exists()) {
        return first;
    }
    for counter in 2.. {
        let path = directory.join(format!("{stem}_{counter}.{extension}"));
        if !written.contains(&path) && (overwrite || !path.exists()) {
            return path;
        }
    }
    unreachable!("the counter loop returns")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::{ExportFormat, ExportResolutionMode};

    fn preset(mode: ExportPresetOutputMode, subfolder: &str, path: &str) -> ExportPreset {
        ExportPreset {
            output_mode: mode,
            output_subfolder: subfolder.to_string(),
            output_path: path.to_string(),
            ..ExportPreset::default()
        }
    }

    #[test]
    fn the_source_subfolder_is_under_the_files_own_folder() {
        assert_eq!(
            resolve_output_dir(
                Path::new("/roll/frame.dng"),
                &preset(ExportPresetOutputMode::SubfolderOfSource, "out", "")
            ),
            PathBuf::from("/roll/out")
        );
        assert_eq!(
            resolve_output_dir(
                Path::new("/roll/frame.dng"),
                &preset(ExportPresetOutputMode::SameAsSource, "out", "/elsewhere")
            ),
            PathBuf::from("/roll")
        );
        assert_eq!(
            resolve_output_dir(
                Path::new("/roll/frame.dng"),
                &preset(ExportPresetOutputMode::Absolute, "", "/elsewhere")
            ),
            PathBuf::from("/elsewhere")
        );
    }

    #[test]
    fn a_second_export_gets_a_new_name_without_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("frame.tiff"), b"old").unwrap();
        let path = unique_path(dir.path(), "frame", "tiff", false, &mut HashSet::new());
        assert_eq!(path.file_name().unwrap(), "frame_2.tiff");
    }

    #[test]
    fn overwrite_keeps_the_first_name() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("frame.tiff"), b"old").unwrap();
        assert_eq!(
            unique_path(dir.path(), "frame", "tiff", true, &mut HashSet::new()),
            dir.path().join("frame.tiff")
        );
    }

    #[test]
    fn an_export_writes_the_tiff_beside_its_source() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("frame.dng");
        std::fs::write(&source, b"x").unwrap();
        let mut config = ExportConfig::default();
        config.export_fmt = ExportFormat::Tiff;
        config.export_resolution_mode = ExportResolutionMode::Original;
        config.paper_aspect_ratio = "original".into();
        config.export_path = dir.path().join("out").display().to_string();
        let workspace = WorkspaceConfig {
            export: config.clone(),
            ..WorkspaceConfig::default()
        };
        let image = ImageBuffer::from_rgb(8, 6, vec![0.5; 8 * 6 * 3]);
        let path = write_export(
            &image,
            &source,
            &config,
            &workspace,
            "sRGB",
            Path::new("/nonexistent"),
        )
        .expect("write");
        assert!(path.is_file(), "{}", path.display());
        assert_eq!(path.extension().unwrap(), "tiff");
        assert_eq!(&std::fs::read(&path).unwrap()[..4], b"II*\0");
    }
}
