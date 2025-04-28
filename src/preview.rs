//! Handles generation of file content previews (text highlighting, images).

use crate::config::AppConfig;
use egui::{text::LayoutJob, Color32, Context, FontId, Sense, TextFormat, TextureHandle, Vec2};
use image::GenericImageView;
use log;
use once_cell::sync::Lazy;
use resvg::usvg; // Remove unused TreeParsing, TreeTextToPath
use std::{fs, path::Path, sync::Arc};
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
    util::LinesWithEndings,
};

/// Default theme name used for light mode syntax highlighting.
pub const DEFAULT_LIGHT_THEME: &str = "base16-ocean.light";
/// Default theme name used for dark mode syntax highlighting.
pub const DEFAULT_DARK_THEME: &str = "base16-ocean.dark";

/// Enum representing the different types of content that can be previewed.
// Removed #[derive(Debug)] because TextureHandle doesn't implement it.
#[derive(Clone)]
pub enum PreviewContent {
    /// Syntax-highlighted text content, represented as an egui `LayoutJob`.
    Text(LayoutJob),
    /// An image, represented by an Arc-wrapped egui `TextureHandle`.
    Image(Arc<TextureHandle>),
    /// An error message indicating why the preview could not be generated.
    Error(String),
    /// Placeholder for content types not yet supported (e.g., PDF, SVG).
    Unsupported(String),
    /// State indicating the preview is currently being loaded.
    Loading,
}

/// Caches the generated preview content for a specific file node.
// Removed #[derive(Debug)] because PreviewContent doesn't implement it.
#[derive(Clone)]
pub struct PreviewCache {
    /// The `FileId` of the node this cache entry belongs to.
    pub node_id: crate::model::FileId,
    /// The actual preview content (text, image, error, etc.).
    pub content: PreviewContent,
    /// The name of the syntect theme used for highlighting (if applicable).
    /// Used for cache invalidation when the application theme changes.
    pub theme_used: Option<String>,
}

// --- Static Assets Initialization ---

// Lazily load syntax definitions once using once_cell.
static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);
// Lazily load theme definitions once using once_cell.
static THEME_SET: Lazy<ThemeSet> = Lazy::new(ThemeSet::load_defaults);

// Optional: PDFium instance (requires careful initialization and thread safety)
// static PDFIUM: Lazy<Mutex<Pdfium>> = Lazy::new(|| Mutex::new(Pdfium::new(...)));

/// Loads and returns static references to the syntax and theme sets.
/// Should be called once during application startup.
pub fn load_syntax_highlighting_assets() -> (&'static SyntaxSet, &'static ThemeSet) {
    // Accessing the Lazy statics initializes them if they haven't been already.
    (Lazy::force(&SYNTAX_SET), Lazy::force(&THEME_SET))
}

// --- Theme and Syntax Selection ---

/// Gets a reference to a `syntect::highlighting::Theme` by name from the loaded `ThemeSet`.
/// Falls back to default light/dark themes if the requested theme is not found.
/// Returns the theme reference and the actual theme name used (which might be the fallback).
fn get_theme<'a>(theme_set: &'a ThemeSet, theme_name: &str) -> (&'a Theme, String) {
    let resolved_name = theme_name.to_lowercase();

    match theme_set.themes.get(&resolved_name) {
        Some(theme) => (theme, resolved_name),
        None => {
            log::warn!(
                "Theme '{}' not found in ThemeSet. Falling back.",
                theme_name
            );
            // Determine fallback based on whether the requested name suggests dark mode
            if theme_name.contains("dark") {
                (
                    theme_set.themes.get(DEFAULT_DARK_THEME).unwrap_or_else(|| {
                        log::error!("Default dark theme '{}' not found!", DEFAULT_DARK_THEME);
                        // Critical fallback: return the first available theme
                        theme_set.themes.values().next().expect("No themes loaded!")
                    }),
                    DEFAULT_DARK_THEME.to_string(),
                )
            } else {
                (
                    theme_set
                        .themes
                        .get(DEFAULT_LIGHT_THEME)
                        .unwrap_or_else(|| {
                            log::error!("Default light theme '{}' not found!", DEFAULT_LIGHT_THEME);
                            // Critical fallback: return the first available theme
                            theme_set.themes.values().next().expect("No themes loaded!")
                        }),
                    DEFAULT_LIGHT_THEME.to_string(),
                )
            }
        }
    }
}

/// Finds the most appropriate `syntect::parsing::SyntaxReference` for a given file path.
/// Uses file extension primarily, with fallbacks based on common extensions and filenames.
/// Defaults to plain text if no specific syntax is found.
fn find_syntax<'a>(syntax_set: &'a SyntaxSet, path: &Path) -> &'a SyntaxReference {
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_lowercase(); // Normalize extension

    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();

    syntax_set
        .find_syntax_by_extension(&extension)
        // Fallbacks based on extension for common config/markup types
        .or_else(|| match extension.as_str() {
            "toml" => syntax_set.find_syntax_by_name("TOML"),
            "yaml" | "yml" => syntax_set.find_syntax_by_name("YAML"),
            "json" => syntax_set.find_syntax_by_name("JSON"),
            "html" | "htm" => syntax_set.find_syntax_by_name("HTML"),
            "xml" => syntax_set.find_syntax_by_name("XML"),
            "md" => syntax_set.find_syntax_by_name("Markdown"),
            // Add more specific extension fallbacks if needed
            _ => None,
        })
        // Fallbacks based on specific filenames
        .or_else(|| match filename {
            "Cargo.lock" => syntax_set.find_syntax_by_name("TOML"),
            "Dockerfile" => syntax_set.find_syntax_by_name("Dockerfile"),
            ".gitignore" => syntax_set.find_syntax_by_name("Git Ignore"),
            // Add more specific filename fallbacks if needed
            _ => None,
        })
        // Final fallback to plain text
        .unwrap_or_else(|| {
            log::trace!(
                "No specific syntax found for '{}', using plain text.",
                path.display()
            );
            syntax_set.find_syntax_plain_text()
        })
}

// --- Preview Generation ---

/// Generates a `PreviewCache` entry for a given file path.
/// This function determines the file type (text, image, unsupported) and calls the
/// appropriate helper function to generate the content. Executed in a background thread.
///
/// # Arguments
/// * `config` - Application configuration (for size limits, theme preference).
/// * `syntax_set` - Reference to the loaded `SyntaxSet`.
/// * `theme_set` - Reference to the loaded `ThemeSet`.
/// * `path` - The absolute path to the file to preview.
/// * `node_id` - The `FileId` associated with this path.
/// * `ctx` - The egui `Context`, needed for creating `TextureHandle`s for images.
///
/// # Returns
/// A `PreviewCache` struct containing the generated content or an error/unsupported message.
pub fn generate_preview(
    config: &AppConfig,
    syntax_set: &'static SyntaxSet,
    theme_set: &'static ThemeSet,
    path: &Path,
    node_id: crate::model::FileId,
    ctx: &Context, // Need context for texture loading
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

    // Determine file type based on extension and generate appropriate preview
    let (content, theme_used) = match extension.as_str() {
        // Image Types
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "tiff" => {
            match generate_image_texture(path, config.max_file_size_preview, ctx) {
                Ok(texture_handle) => (PreviewContent::Image(Arc::new(texture_handle)), None),
                Err(e) => (PreviewContent::Error(e), None),
            }
        }
        // SVG Type
        "svg" => match generate_svg_texture(path, config.max_file_size_preview, ctx) {
            Ok(texture_handle) => (PreviewContent::Image(Arc::new(texture_handle)), None),
            Err(e) => (PreviewContent::Error(e), None),
        },
        // Unsupported Types (Explicitly listed)
        "pdf" => (
            PreviewContent::Unsupported("PDF preview not yet implemented".to_string()),
            None,
        ),
        // TODO: Add other known non-text/non-image types here if desired

        // Default to Text Type
        _ => {
            match highlight_text_content(config, syntax_set, theme_set, path) {
                Ok((job, theme_name)) => (PreviewContent::Text(job), Some(theme_name)),
                Err(e) => {
                    // If text highlighting fails, determine the theme that *would* have been used
                    // for cache consistency checks.
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

/// Reads text file content, performs syntax highlighting, and returns an egui `LayoutJob`.
/// Also returns the name of the syntect theme used for highlighting.
fn highlight_text_content(
    config: &AppConfig,
    syntax_set: &'static SyntaxSet,
    theme_set: &'static ThemeSet,
    path: &Path,
) -> Result<(LayoutJob, String), String> {
    // 1. Read file content (handles size limit and basic encoding fallback)
    let content = read_file_content(path, config.max_file_size_preview)?;
    if content.is_empty() {
        // Return an empty LayoutJob for empty files
        return Ok((LayoutJob::default(), get_fallback_theme_name(config)));
    }

    // 2. Determine Syntax and Theme
    let syntax = find_syntax(syntax_set, path);
    log::trace!("Using syntax '{}' for '{}'", syntax.name, path.display());

    let theme_choice = match config.theme.as_str() {
        "dark" => DEFAULT_DARK_THEME,
        "light" => DEFAULT_LIGHT_THEME,
        _ => match dark_light::detect() {
            // System theme
            Ok(dark_light::Mode::Dark) => DEFAULT_DARK_THEME,
            _ => DEFAULT_LIGHT_THEME,
        },
    };
    let (theme, theme_name_used) = get_theme(theme_set, theme_choice);
    log::trace!("Using theme '{}' for '{}'", theme_name_used, path.display());

    // 3. Perform Highlighting and Build LayoutJob
    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut job = LayoutJob::default();
    let font_id = FontId::monospace(12.0); // Use a consistent monospace font

    // Calculate width needed for line numbers
    let line_count = content.lines().count();
    let line_number_width = if line_count == 0 {
        1
    } else {
        (line_count as f64).log10() as usize + 1
    };

    for (i, line) in LinesWithEndings::from(&content).enumerate() {
        // Add line number (right-aligned)
        let line_num_str = format!("{:<width$} │ ", i + 1, width = line_number_width);
        job.append(
            &line_num_str,
            0.0, // Initial offset ( galley calculates actual position)
            TextFormat {
                font_id: font_id.clone(),
                color: Color32::GRAY,        // Dim color for line numbers
                valign: egui::Align::BOTTOM, // Align baseline
                ..Default::default()
            },
        );

        // Highlight the line content
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
                    // let bold = style.font_style.contains(syntect::highlighting::FontStyle::BOLD); // Not directly supported by TextFormat

                    job.append(
                        text,
                        0.0,
                        TextFormat {
                            font_id: font_id.clone(),
                            color,
                            italics,
                            underline: if underline {
                                egui::Stroke::new(1.0, color)
                            } else {
                                egui::Stroke::NONE
                            },
                            // background: if style.background != theme.settings.background.unwrap_or_default() { Color32::from_rgb(style.background.r, style.background.g, style.background.b) } else { Color32::TRANSPARENT },
                            valign: egui::Align::BOTTOM, // Align baseline
                            ..Default::default()
                        },
                    );
                }
            }
            Err(e) => {
                // If highlighting fails for a line, append the raw line with an error color
                log::error!("Syntect highlighting error on line {}: {}", i + 1, e);
                job.append(
                    line, // Append the original line content
                    0.0,
                    TextFormat {
                        font_id: font_id.clone(),
                        color: Color32::RED, // Indicate error
                        valign: egui::Align::BOTTOM,
                        ..Default::default()
                    },
                );
            }
        }
    }

    Ok((job, theme_name_used))
}

/// Reads file content into a String, respecting the maximum size limit.
/// Attempts UTF-8 decoding, falling back to lossy decoding if necessary.
/// Returns an error string if the file exceeds the size limit or cannot be read.
pub(crate) fn read_file_content(path: &Path, max_size: i64) -> Result<String, String> {
    let bytes = read_file_bytes(path, max_size)?; // Use helper to read bytes first

    // Attempt UTF-8 decoding
    match String::from_utf8(bytes) {
        Ok(content) => Ok(content),
        Err(e) => {
            // If UTF-8 fails, try lossy decoding
            log::warn!(
                "UTF-8 decoding failed for '{}', trying lossy decoding. Error: {}",
                path.display(),
                e
            );
            // Get the bytes back from the error and perform lossy conversion
            let bytes_lossy = e.into_bytes();
            Ok(String::from_utf8_lossy(&bytes_lossy).to_string())
        }
    }
}

/// Reads file content into a byte vector, respecting the maximum size limit.
/// Returns an error string if the file exceeds the limit or cannot be read.
fn read_file_bytes(path: &Path, max_size: i64) -> Result<Vec<u8>, String> {
    // 1. Get metadata to check size first
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

    // 2. Check size limit
    let use_limit = max_size >= 0; // Treat negative max_size as unlimited
    let max_size_u64 = if use_limit { max_size as u64 } else { u64::MAX };

    if use_limit && metadata.len() > max_size_u64 {
        let limit_str = humansize::format_size(max_size_u64, humansize::DECIMAL);
        let file_size_str = humansize::format_size(metadata.len(), humansize::DECIMAL);
        return Err(format!(
            "File size ({}) exceeds maximum preview limit ({})",
            file_size_str, limit_str
        ));
    }

    // Handle empty files explicitly
    if metadata.len() == 0 {
        return Ok(Vec::new());
    }

    // 3. Read file bytes
    match fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(e) => Err(format!("Failed to read file '{}': {}", path.display(), e)),
    }
}

/// Loads an image file, converts it to an egui `TextureHandle`, respecting size limits.
fn generate_image_texture(
    path: &Path,
    max_size: i64,
    ctx: &Context, // Need egui context to load texture
) -> Result<TextureHandle, String> {
    log::debug!("Generating image texture for: {}", path.display());
    let bytes = read_file_bytes(path, max_size)?;
    if bytes.is_empty() {
        return Err("Image file is empty or could not be read".to_string());
    }

    // Load image using the image crate
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
    log::trace!(
        "Image loaded successfully, dimensions: {:?}",
        img.dimensions()
    );

    // Convert to egui::ColorImage (needs RGBA format)
    let size = [img.width() as _, img.height() as _];
    let rgba_image = img.to_rgba8(); // Convert to RGBA format
    let pixels = rgba_image.into_raw(); // Get flat pixel data Vec<u8>

    // Create egui ColorImage (ensure correct pixel order and format)
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);

    // Create TextureHandle using egui context's texture manager
    let texture_options = egui::TextureOptions::LINEAR; // Use linear filtering for better scaling
    let texture_handle = ctx.load_texture(
        path.display().to_string(), // Debug name for the texture
        color_image,
        texture_options,
    );

    log::debug!(
        "Image texture generated successfully for: {}",
        path.display()
    );
    Ok(texture_handle)
}

/// Loads an SVG file, renders it using resvg, and converts it to an egui `TextureHandle`.
fn generate_svg_texture(
    path: &Path,
    max_size: i64,
    ctx: &Context, // Need egui context to load texture
) -> Result<TextureHandle, String> {
    log::debug!("Generating SVG texture for: {}", path.display());
    let bytes = read_file_bytes(path, max_size)?;
    if bytes.is_empty() {
        return Err("SVG file is empty or could not be read".to_string());
    }

    // Load the font database.
    let mut fontdb = usvg::fontdb::Database::new();
    fontdb.load_system_fonts(); // Attempt to load system fonts

    // Set up parsing options with the font database
    let opts = usvg::Options {
        fontdb: Arc::new(fontdb), // Pass the font database via options
        ..Default::default()
    };

    // Parse SVG using usvg with options
    let tree = usvg::Tree::from_data(&bytes, &opts)
        .map_err(|e| format!("Failed to parse SVG '{}': {}", path.display(), e))?;

    // Font loading is handled during parsing via options, no separate call needed.

    let tree_size = tree.size();
    let width = tree_size.width().ceil() as u32;
    let height = tree_size.height().ceil() as u32;

    if width == 0 || height == 0 {
        return Err("SVG has zero width or height".to_string());
    }

    // Create a pixel buffer (pixmap) to render onto
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| format!("Failed to create pixel map for SVG ({}x{})", width, height))?;

    // Render the SVG tree onto the pixmap using correct arguments
    resvg::render(
        &tree,
        usvg::Transform::identity(), // Use usvg::Transform
        &mut pixmap.as_mut(), // Pass mutable reference to PixmapMut
    );
    // render function returns () on success, errors are typically handled during parsing/pixmap creation

    // Convert the pixmap data (RGBA) to egui::ColorImage
    let pixels = pixmap.take(); // Take ownership of the pixel data Vec<u8>
    let size = [width as usize, height as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);

    // Create TextureHandle using egui context's texture manager
    let texture_options = egui::TextureOptions::LINEAR; // Use linear filtering
    let texture_handle = ctx.load_texture(
        path.display().to_string(), // Debug name for the texture
        color_image,
        texture_options,
    );

    log::debug!(
        "SVG texture generated successfully for: {}",
        path.display()
    );
    Ok(texture_handle)
}


// --- Helper Functions ---

/// Helper to get the effective theme name based on AppConfig.
/// Used for determining theme for error messages or cache validation.
fn get_fallback_theme_name(config: &AppConfig) -> String {
    match config.theme.as_str() {
        "dark" => DEFAULT_DARK_THEME.to_string(),
        "light" => DEFAULT_LIGHT_THEME.to_string(),
        _ => match dark_light::detect() {
            // System theme
            Ok(dark_light::Mode::Dark) => DEFAULT_DARK_THEME.to_string(),
            _ => DEFAULT_LIGHT_THEME.to_string(),
        },
    }
}

/// Helper function to render a `PreviewContent` enum variant into the UI.
/// Handles text layout, image scaling, and error/unsupported messages.
pub(crate) fn render_preview_content(ui: &mut egui::Ui, content: &PreviewContent) {
    match content {
        PreviewContent::Text(layout_job) => {
            // Allow text selection by making the label sensitive
            ui.add(egui::Label::new(layout_job.clone()).sense(Sense::click_and_drag()));
        }
        PreviewContent::Image(texture_handle_arc) => {
            // Calculate max width/height for the image based on available panel size
            let max_size = ui.available_size() - Vec2::splat(10.0); // Leave some padding

            if max_size.x <= 0.0 || max_size.y <= 0.0 {
                ui.label("[Panel too small to display image]");
                return;
            }

            // Get image dimensions from the texture handle
            let img_size = texture_handle_arc.size_vec2();

            // Calculate scaled size to fit within max_size while maintaining aspect ratio
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

            // Ensure display size is positive
            display_size = display_size.max(Vec2::splat(1.0));

            // Display the image using the TextureHandle's ID and the calculated size
            ui.image((texture_handle_arc.id(), display_size));
        }
        PreviewContent::Error(err_msg) => {
            ui.colored_label(Color32::RED, format!("Preview Error: {}", err_msg));
        }
        PreviewContent::Unsupported(msg) => {
            ui.label(format!("Preview not available: {}", msg));
        }
        PreviewContent::Loading => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Loading preview...");
            });
        }
    }
}
