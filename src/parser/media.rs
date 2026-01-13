use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use open::that;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Audio,
    Video,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Media {
    label: String,
    path: PathBuf,
    kind: MediaKind,
}

impl Media {
    pub fn play(&self) -> Result<()> {
        if !self.path.is_file() || !self.path.exists() {
            return Err(anyhow!("File does not exist: {}", self.path.display()));
        }
        that(&self.path)?;
        Ok(())
    }
}

fn media_kind_from_path(path: &Path) -> Option<MediaKind> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())?;

    match ext.as_str() {
        // Images
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" => Some(MediaKind::Image),

        // Audio
        "mp3" | "wav" | "ogg" | "flac" | "m4a" => Some(MediaKind::Audio),

        // Video
        "mp4" | "webm" | "mkv" | "mov" | "avi" => Some(MediaKind::Video),

        _ => None,
    }
}

fn resolve_media_path(path: PathBuf, base_dir: Option<&Path>) -> PathBuf {
    if path.is_relative()
        && let Some(dir) = base_dir
    {
        return dir.join(path);
    }
    path
}

pub fn extract_media(markdown: &str, base_dir: Option<&Path>) -> Vec<Media> {
    let parser = Parser::new(markdown);

    let mut media = Vec::new();

    let mut current_path: Option<PathBuf> = None;
    let mut current_kind: Option<MediaKind> = None;
    let mut current_label = String::new();

    for event in parser {
        match event {
            // [label](path)
            Event::Start(Tag::Link { dest_url, .. }) => {
                let resolved_path = resolve_media_path(PathBuf::from(dest_url.as_ref()), base_dir);
                if let Some(kind) = media_kind_from_path(&resolved_path) {
                    current_path = Some(resolved_path);
                    current_kind = Some(kind);
                    current_label.clear();
                }
            }

            // ![alt](path)
            Event::Start(Tag::Image { dest_url, .. }) => {
                let resolved_path = resolve_media_path(PathBuf::from(dest_url.as_ref()), base_dir);
                if let Some(kind) = media_kind_from_path(&resolved_path) {
                    media.push(Media {
                        label: "image".to_string(),
                        path: resolved_path,
                        kind,
                    });
                }
            }

            Event::Text(text) => {
                if current_path.is_some() {
                    current_label.push_str(&text);
                }
            }

            Event::End(TagEnd::Link) => {
                if let (Some(path), Some(kind)) = (current_path.take(), current_kind.take()) {
                    media.push(Media {
                        label: if current_label.is_empty() {
                            path.file_name()
                                .and_then(|f| f.to_str())
                                .unwrap_or("media")
                                .to_string()
                        } else {
                            current_label.clone()
                        },
                        path,
                        kind,
                    });
                }
                current_label.clear();
            }

            _ => {}
        }
    }

    media
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::parser::{Media, MediaKind};

    use super::extract_media;

    #[test]
    fn test_markdown_parsing() {
        let contents = "# Sample Card

What animal is this?

![dog](media/dog.jpg)

Listen to the pronunciation:
[audio](media/dog.mp3)

Watch the clip:
[video](media/dog.mp4)

This is a normal link and should be ignored:
[example](https://example.com)";
        let medias = extract_media(contents, None);
        let expected = vec![
            Media {
                label: "image".to_string(),
                path: PathBuf::from("media/dog.jpg"),
                kind: MediaKind::Image,
            },
            Media {
                label: "audio".to_string(),
                path: PathBuf::from("media/dog.mp3"),
                kind: MediaKind::Audio,
            },
            Media {
                label: "video".to_string(),
                path: PathBuf::from("media/dog.mp4"),
                kind: MediaKind::Video,
            },
        ];

        assert_eq!(medias, expected);
    }

    #[test]
    fn resolves_relative_paths_with_base_dir() {
        let contents = "![dog](media/dog.jpg)\n[audio](../audio/bark.mp3)";
        let medias = extract_media(contents, Some(Path::new("notes/cards")));
        let expected = vec![
            Media {
                label: "image".to_string(),
                path: PathBuf::from("notes/cards/media/dog.jpg"),
                kind: MediaKind::Image,
            },
            Media {
                label: "audio".to_string(),
                path: PathBuf::from("notes/cards/../audio/bark.mp3"),
                kind: MediaKind::Audio,
            },
        ];
        assert_eq!(medias, expected);
    }
}
