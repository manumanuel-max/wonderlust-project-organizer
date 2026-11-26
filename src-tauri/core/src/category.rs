use serde::{Deserialize, Serialize};

/// Las cinco categorías de la estructura estandarizada de Wonderlust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Imagen,
    Video,
    Audio,
    Editable,
    Archivos,
}

impl Category {
    pub const ALL: [Category; 5] = [
        Category::Imagen,
        Category::Video,
        Category::Audio,
        Category::Editable,
        Category::Archivos,
    ];

    /// Nombre de la carpeta en disco, p. ej. `1. imagen`.
    pub fn folder_name(self) -> &'static str {
        match self {
            Category::Imagen => "1. imagen",
            Category::Video => "2. video",
            Category::Audio => "3. audio",
            Category::Editable => "4. editable",
            Category::Archivos => "5. archivos",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Category::Imagen => "Imagen",
            Category::Video => "Video",
            Category::Audio => "Audio",
            Category::Editable => "Editable",
            Category::Archivos => "Archivos",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Category::Imagen => "imagen",
            Category::Video => "video",
            Category::Audio => "audio",
            Category::Editable => "editable",
            Category::Archivos => "archivos",
        }
    }

    /// Reconoce el nombre de una carpeta de categoría estandarizada
    /// (`1. imagen`, `1.imagen`, `1 - Imagen`, ...).
    pub fn from_folder_name(name: &str) -> Option<Category> {
        let lower = name.trim().to_lowercase();
        let digits: String = lower.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            return None;
        }
        let rest = lower[digits.len()..]
            .trim_start_matches(|c: char| c == '.' || c == '-' || c == '_' || c.is_whitespace());
        let cat = match rest {
            "imagen" | "imagenes" | "imágenes" => Category::Imagen,
            "video" | "videos" | "vídeo" => Category::Video,
            "audio" | "audios" => Category::Audio,
            "editable" | "editables" => Category::Editable,
            "archivos" | "archivo" => Category::Archivos,
            _ => return None,
        };
        let expected = match cat {
            Category::Imagen => "1",
            Category::Video => "2",
            Category::Audio => "3",
            Category::Editable => "4",
            Category::Archivos => "5",
        };
        if digits == expected {
            Some(cat)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_names_are_standard() {
        assert_eq!(Category::Imagen.folder_name(), "1. imagen");
        assert_eq!(Category::Archivos.folder_name(), "5. archivos");
    }

    #[test]
    fn recognizes_standard_folders() {
        assert_eq!(Category::from_folder_name("1. imagen"), Some(Category::Imagen));
        assert_eq!(Category::from_folder_name("2.Video"), Some(Category::Video));
        assert_eq!(Category::from_folder_name("4 - Editable"), Some(Category::Editable));
        assert_eq!(Category::from_folder_name("3. video"), None);
        assert_eq!(Category::from_folder_name("imagen"), None);
    }
}
