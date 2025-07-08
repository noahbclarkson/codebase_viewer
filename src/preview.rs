//! Handles generation of file content previews (text highlighting, images).

use crate::config::AppConfig;
use egui::{text::LayoutJob, Color32, Context, FontId, Sense, TextureHandle, Vec2};
use egui_phosphor::regular::*;
use log;
use once_cell::sync::Lazy;
use resvg::usvg;
use std::{fs, path::Path, sync::Arc};
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
    util::LinesWithEndings,
};

pub const DEFAULT_LIGHT_THEME: &str = "base16-ocean.light";
pub const DEFAULT_DARK_THEME: &str = "base16-ocean.dark";

#[derive(Clone)]
pub enum PreviewContent {
    Text(LayoutJob),
    Image(Arc<TextureHandle>),
    Error(String),
    Unsupported(String),
    Loading,
}

#[derive(Clone)]
pub struct PreviewCache {
    pub node_id: crate::model::FileId,
    pub content: PreviewContent,
    pub theme_used: Option<String>,
}

static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: Lazy<ThemeSet> = Lazy::new(ThemeSet::load_defaults);

pub fn load_syntax_highlighting_assets() -> (&'static SyntaxSet, &'static ThemeSet) {
    (Lazy::force(&SYNTAX_SET), Lazy::force(&THEME_SET))
}

fn get_theme<'a>(theme_set: &'a ThemeSet, theme_name: &str) -> (&'a Theme, String) {
    let resolved_name = theme_name.to_lowercase();
    match theme_set.themes.get(&resolved_name) {
        Some(theme) => (theme, resolved_name),
        None => {
            log::warn!("Theme '{theme_name}' not found. Falling back.");
            if theme_name.contains("dark") {
                (
                    theme_set.themes.get(DEFAULT_DARK_THEME).unwrap(),
                    DEFAULT_DARK_THEME.to_string(),
                )
            } else {
                (
                    theme_set.themes.get(DEFAULT_LIGHT_THEME).unwrap(),
                    DEFAULT_LIGHT_THEME.to_string(),
                )
            }
        }
    }
}

fn find_syntax<'a>(syntax_set: &'a SyntaxSet, path: &Path) -> &'a SyntaxReference {
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    syntax_set
        .find_syntax_by_extension(&extension)
        .or_else(|| match extension.as_str() {
            "toml" => syntax_set.find_syntax_by_name("TOML"),
            "yaml" | "yml" => syntax_set.find_syntax_by_name("YAML"),
            "json" => syntax_set.find_syntax_by_name("JSON"),
            "html" | "htm" => syntax_set.find_syntax_by_name("HTML"),
            "xml" => syntax_set.find_syntax_by_name("XML"),
            "md" => syntax_set.find_syntax_by_name("Markdown"),
            _ => None,
        })
        .or_else(|| match filename {
            "Cargo.lock" => syntax_set.find_syntax_by_name("TOML"),
            "Dockerfile" => syntax_set.find_syntax_by_name("Dockerfile"),
            ".gitignore" => syntax_set.find_syntax_by_name("Git Ignore"),
            _ => None,
        })
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
}

pub fn generate_preview(
    config: &AppConfig,
    syntax_set: &'static SyntaxSet,
    theme_set: &'static ThemeSet,
    path: &Path,
    node_id: crate::model::FileId,
    ctx: &Context,
) -> PreviewCache {
    log::debug!(
        "Generating preview for node {}, path: {}",
        node_id,
        path.display()
    );
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    let (content, theme_used) = match extension.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "tiff" => {
            match generate_image_texture(path, config.max_file_size_preview, ctx) {
                Ok(texture_handle) => (PreviewContent::Image(Arc::new(texture_handle)), None),
                Err(e) => (PreviewContent::Error(e), None),
            }
        }
        "svg" => match generate_svg_texture(path, config.max_file_size_preview, ctx) {
            Ok(texture_handle) => (PreviewContent::Image(Arc::new(texture_handle)), None),
            Err(e) => (PreviewContent::Error(e), None),
        },
        "pdf" => (
            PreviewContent::Unsupported("PDF preview not yet implemented".to_string()),
            None,
        ),
        _ => {
            // The word_wrap setting is handled by the UI when rendering, not during generation.
            match highlight_text_content(config, syntax_set, theme_set, path) {
                Ok((job, theme_name)) => (PreviewContent::Text(job), Some(theme_name)),
                Err(e) => {
                    let fallback_theme = get_fallback_theme_name(config);
                    (PreviewContent::Error(e), Some(fallback_theme))
                }
            }
        }
    };

    PreviewCache {
        node_id,
        content,
        theme_used,
    }
}

fn highlight_text_content(
    config: &AppConfig,
    syntax_set: &'static SyntaxSet,
    theme_set: &'static ThemeSet,
    path: &Path,
) -> Result<(LayoutJob, String), String> {
    let content = read_file_content(path, config.max_file_size_preview)?;
    if content.is_empty() {
        return Ok((LayoutJob::default(), get_fallback_theme_name(config)));
    }

    let syntax = find_syntax(syntax_set, path);
    let theme_choice = match config.theme.as_str() {
        "dark" => DEFAULT_DARK_THEME,
        "light" => DEFAULT_LIGHT_THEME,
        _ => match dark_light::detect() {
            Ok(dark_light::Mode::Dark) => DEFAULT_DARK_THEME,
            _ => DEFAULT_LIGHT_THEME,
        },
    };
    let (theme, theme_name_used) = get_theme(theme_set, theme_choice);

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut job = LayoutJob::default();
    let font_id = FontId::monospace(12.0);
    let line_count = content.lines().count();
    let line_number_width = if line_count == 0 {
        1
    } else {
        (line_count as f64).log10() as usize + 1
    };

    for (i, line) in LinesWithEndings::from(&content).enumerate() {
        let line_num_str = format!("{:<width$} │ ", i + 1, width = line_number_width);
        job.append(
            &line_num_str,
            0.0,
            egui::TextFormat {
                font_id: font_id.clone(),
                color: Color32::GRAY,
                valign: egui::Align::BOTTOM,
                ..Default::default()
            },
        );

        match highlighter.highlight_line(line, syntax_set) {
            Ok(ranges) => {
                for (style, text) in ranges {
                    let color = Color32::from_rgb(
                        style.foreground.r,
                        style.foreground.g,
                        style.foreground.b,
                    );
                    let italics = style
                        .font_style
                        .contains(syntect::highlighting::FontStyle::ITALIC);
                    let underline = style
                        .font_style
                        .contains(syntect::highlighting::FontStyle::UNDERLINE);
                    job.append(
                        text,
                        0.0,
                        egui::TextFormat {
                            font_id: font_id.clone(),
                            color,
                            italics,
                            underline: if underline {
                                egui::Stroke::new(1.0, color)
                            } else {
                                egui::Stroke::NONE
                            },
                            valign: egui::Align::BOTTOM,
                            ..Default::default()
                        },
                    );
                }
            }
            Err(e) => {
                log::error!("Syntect highlighting error on line {}: {}", i + 1, e);
                job.append(
                    line,
                    0.0,
                    egui::TextFormat {
                        font_id: font_id.clone(),
                        color: Color32::RED,
                        valign: egui::Align::BOTTOM,
                        ..Default::default()
                    },
                );
            }
        }
    }

    Ok((job, theme_name_used))
}

pub(crate) fn read_file_content(path: &Path, max_size: i64) -> Result<String, String> {
    let bytes = read_file_bytes(path, max_size)?;
    match String::from_utf8(bytes) {
        Ok(content) => Ok(content),
        Err(e) => {
            log::warn!(
                "UTF-8 decoding failed for '{}', trying lossy decoding. Error: {}",
                path.display(),
                e
            );
            let bytes_lossy = e.into_bytes();
            Ok(String::from_utf8_lossy(&bytes_lossy).to_string())
        }
    }
}

fn read_file_bytes(path: &Path, max_size: i64) -> Result<Vec<u8>, String> {
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) => {
            return Err(format!(
                "Failed to get metadata for '{}': {}",
                path.display(),
                e
            ))
        }
    };
    let use_limit = max_size >= 0;
    let max_size_u64 = if use_limit { max_size as u64 } else { u64::MAX };
    if use_limit && metadata.len() > max_size_u64 {
        let limit_str = humansize::format_size(max_size_u64, humansize::DECIMAL);
        let file_size_str = humansize::format_size(metadata.len(), humansize::DECIMAL);
        return Err(format!(
            "File size ({file_size_str}) exceeds maximum preview limit ({limit_str})"
        ));
    }
    if metadata.len() == 0 {
        return Ok(Vec::new());
    }
    match fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(e) => Err(format!("Failed to read file '{}': {}", path.display(), e)),
    }
}

fn generate_image_texture(
    path: &Path,
    max_size: i64,
    ctx: &Context,
) -> Result<TextureHandle, String> {
    log::debug!("Generating image texture for: {}", path.display());
    let bytes = read_file_bytes(path, max_size)?;
    if bytes.is_empty() {
        return Err("Image file is empty or could not be read".to_string());
    }
    let img = match image::load_from_memory(&bytes) {
        Ok(img) => img,
        Err(e) => {
            return Err(format!(
                "Failed to decode image '{}': {}",
                path.display(),
                e
            ))
        }
    };
    let size = [img.width() as _, img.height() as _];
    let rgba_image = img.to_rgba8();
    let pixels = rgba_image.into_raw();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
    let texture_options = egui::TextureOptions::LINEAR;
    let texture_handle = ctx.load_texture(path.display().to_string(), color_image, texture_options);
    Ok(texture_handle)
}

fn generate_svg_texture(
    path: &Path,
    max_size: i64,
    ctx: &Context,
) -> Result<TextureHandle, String> {
    log::debug!("Generating SVG texture for: {}", path.display());
    let bytes = read_file_bytes(path, max_size)?;
    if bytes.is_empty() {
        return Err("SVG file is empty or could not be read".to_string());
    }
    let mut fontdb = usvg::fontdb::Database::new();
    fontdb.load_system_fonts();
    let opts = usvg::Options {
        fontdb: Arc::new(fontdb),
        ..Default::default()
    };
    let tree = usvg::Tree::from_data(&bytes, &opts)
        .map_err(|e| format!("Failed to parse SVG '{}': {}", path.display(), e))?;
    let tree_size = tree.size();
    let width = tree_size.width().ceil() as u32;
    let height = tree_size.height().ceil() as u32;
    if width == 0 || height == 0 {
        return Err("SVG has zero width or height".to_string());
    }
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| format!("Failed to create pixel map for SVG ({width}x{height})"))?;
    resvg::render(&tree, usvg::Transform::identity(), &mut pixmap.as_mut());
    let pixels = pixmap.take();
    let size = [width as usize, height as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
    let texture_options = egui::TextureOptions::LINEAR;
    let texture_handle = ctx.load_texture(path.display().to_string(), color_image, texture_options);
    Ok(texture_handle)
}

fn get_fallback_theme_name(config: &AppConfig) -> String {
    match config.theme.as_str() {
        "dark" => DEFAULT_DARK_THEME.to_string(),
        "light" => DEFAULT_LIGHT_THEME.to_string(),
        _ => match dark_light::detect() {
            Ok(dark_light::Mode::Dark) => DEFAULT_DARK_THEME.to_string(),
            _ => DEFAULT_LIGHT_THEME.to_string(),
        },
    }
}

/// Helper function to render a `PreviewContent` enum variant into the UI.
pub(crate) fn render_preview_content(ui: &mut egui::Ui, content: &PreviewContent, word_wrap: bool) {
    match content {
        PreviewContent::Text(layout_job) => {
            let mut job = layout_job.clone();
            job.wrap.break_anywhere = word_wrap;
            // Let the UI decide the max width when wrapping is enabled.
            job.wrap.max_width = if word_wrap {
                ui.available_width()
            } else {
                f32::INFINITY
            };
            ui.add(egui::Label::new(job).sense(Sense::click_and_drag()));
        }
        PreviewContent::Image(texture_handle_arc) => {
            let max_size = ui.available_size() - Vec2::splat(10.0);
            if max_size.x <= 0.0 || max_size.y <= 0.0 {
                ui.label("[Panel too small to display image]");
                return;
            }
            let img_size = texture_handle_arc.size_vec2();
            let aspect_ratio = img_size.x / img_size.y;
            let mut display_size = img_size;
            if display_size.x > max_size.x {
                display_size.x = max_size.x;
                display_size.y = display_size.x / aspect_ratio;
            }
            if display_size.y > max_size.y {
                display_size.y = max_size.y;
                display_size.x = display_size.y * aspect_ratio;
            }
            display_size = display_size.max(Vec2::splat(1.0));
            ui.image((texture_handle_arc.id(), display_size));
        }
        PreviewContent::Error(err_msg) => {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.3);
                ui.label(
                    egui::RichText::new(WARNING_CIRCLE)
                        .size(48.0)
                        .color(ui.visuals().warn_fg_color),
                );
                ui.label(egui::RichText::new("Preview Error").strong());
                ui.label(err_msg);
            });
        }
        PreviewContent::Unsupported(msg) => {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.3);
                ui.label(
                    egui::RichText::new(FILE)
                        .size(48.0)
                        .color(ui.visuals().weak_text_color()),
                );
                ui.label(egui::RichText::new("Unsupported File Type").strong());
                ui.label(msg);
            });
        }
        PreviewContent::Loading => {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
        }
    }
}
