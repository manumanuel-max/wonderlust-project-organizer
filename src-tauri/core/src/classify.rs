//! Clasificación por extensión (sin distinguir mayúsculas/minúsculas).

use crate::Category;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

pub const IMAGE_EXTS: &[&str] = &[
    "jpg", "jpeg", "png", "tiff", "tif", "webp", "heic", "exr", "gif", "bmp", "tga", "dpx", "dng",
    "cr2", "cr3", "nef", "arw",
];
pub const VIDEO_EXTS: &[&str] = &[
    "mp4", "mov", "mxf", "avi", "mkv", "r3d", "braw", "m4v", "webm", "mpg", "mpeg", "mts", "m2ts",
    "prores",
];
pub const AUDIO_EXTS: &[&str] = &["wav", "mp3", "aac", "m4a", "aiff", "aif", "flac", "ogg", "wma"];
/// La categoría editable es EXCLUSIVA de After Effects y Premiere Pro.
pub const EDITABLE_EXTS: &[&str] = &["aep", "aepx", "prproj"];
/// Ejemplos documentados de la categoría archivos (cualquier desconocida también cae aquí).
pub const ARCHIVOS_EXTS: &[&str] = &[
    "ai", "psd", "psb", "eps", "svg", "pdf", "indd", "c4d", "blend", "drp", "doc", "docx", "xls",
    "xlsx", "ppt", "pptx", "txt", "json", "csv", "zip", "rar", "7z", "srt", "otf", "ttf", "lut",
    "cube",
];

/// Regla adicional configurada por el usuario.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomRule {
    pub extension: String,
    pub category: Category,
}

/// Resultado de validar una regla personalizada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleRejection {
    /// No se puede asignar nada a "editable" desde reglas personalizadas.
    EditableIsReserved,
    /// No se pueden reasignar AEP/AEPX/PRPROJ fuera de "editable".
    ProtectedExtension,
    EmptyExtension,
}

pub fn normalize_ext(ext: &str) -> String {
    ext.trim().trim_start_matches('.').to_lowercase()
}

pub fn extension_of(path: &Path) -> String {
    path.extension()
        .map(|e| normalize_ext(&e.to_string_lossy()))
        .unwrap_or_default()
}

pub fn is_adobe_project_ext(ext: &str) -> bool {
    EDITABLE_EXTS.contains(&normalize_ext(ext).as_str())
}

pub fn validate_rule(rule: &CustomRule) -> Result<(), RuleRejection> {
    let ext = normalize_ext(&rule.extension);
    if ext.is_empty() {
        return Err(RuleRejection::EmptyExtension);
    }
    if EDITABLE_EXTS.contains(&ext.as_str()) {
        return Err(RuleRejection::ProtectedExtension);
    }
    if rule.category == Category::Editable {
        return Err(RuleRejection::EditableIsReserved);
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Classifier {
    map: HashMap<String, Category>,
}

impl Default for Classifier {
    fn default() -> Self {
        Self::new(&[])
    }
}

impl Classifier {
    /// Construye el clasificador. Las reglas personalizadas inválidas
    /// (las que tocarían la exclusividad de "editable") se ignoran.
    pub fn new(custom: &[CustomRule]) -> Self {
        let mut map = HashMap::new();
        for e in IMAGE_EXTS {
            map.insert(e.to_string(), Category::Imagen);
        }
        for e in VIDEO_EXTS {
            map.insert(e.to_string(), Category::Video);
        }
        for e in AUDIO_EXTS {
            map.insert(e.to_string(), Category::Audio);
        }
        for e in ARCHIVOS_EXTS {
            map.insert(e.to_string(), Category::Archivos);
        }
        for rule in custom {
            if validate_rule(rule).is_ok() {
                map.insert(normalize_ext(&rule.extension), rule.category);
            }
        }
        for e in EDITABLE_EXTS {
            map.insert(e.to_string(), Category::Editable);
        }
        Classifier { map }
    }

    pub fn classify_ext(&self, ext: &str) -> Category {
        self.map
            .get(&normalize_ext(ext))
            .copied()
            .unwrap_or(Category::Archivos)
    }

    /// `true` si la extensión no tiene una regla explícita (cae en archivos por defecto).
    pub fn is_unknown(&self, ext: &str) -> bool {
        !self.map.contains_key(&normalize_ext(ext))
    }

    pub fn classify_path(&self, path: &Path) -> Category {
        self.classify_ext(&extension_of(path))
    }

    /// Una categoría es válida para un archivo si respeta la exclusividad de "editable".
    pub fn category_allowed(&self, ext: &str, category: Category) -> bool {
        let adobe = is_adobe_project_ext(ext);
        match category {
            Category::Editable => adobe,
            _ => !adobe,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_all_documented_extensions() {
        let c = Classifier::default();
        for e in IMAGE_EXTS {
            assert_eq!(c.classify_ext(e), Category::Imagen, "{e}");
        }
        for e in VIDEO_EXTS {
            assert_eq!(c.classify_ext(e), Category::Video, "{e}");
        }
        for e in AUDIO_EXTS {
            assert_eq!(c.classify_ext(e), Category::Audio, "{e}");
        }
        for e in EDITABLE_EXTS {
            assert_eq!(c.classify_ext(e), Category::Editable, "{e}");
        }
        for e in ARCHIVOS_EXTS {
            assert_eq!(c.classify_ext(e), Category::Archivos, "{e}");
        }
    }

    #[test]
    fn is_case_insensitive() {
        let c = Classifier::default();
        assert_eq!(c.classify_path(Path::new("A/B/CLIP.MOV")), Category::Video);
        assert_eq!(c.classify_path(Path::new("foto.JpEg")), Category::Imagen);
        assert_eq!(c.classify_path(Path::new("Proyecto.AEP")), Category::Editable);
        assert_eq!(c.classify_path(Path::new("Edit.PrProj")), Category::Editable);
        assert_eq!(c.classify_ext(".WAV"), Category::Audio);
    }

    #[test]
    fn unknown_and_missing_extensions_go_to_archivos() {
        let c = Classifier::default();
        assert_eq!(c.classify_ext("xyz123"), Category::Archivos);
        assert!(c.is_unknown("xyz123"));
        assert_eq!(c.classify_path(Path::new("README")), Category::Archivos);
        assert_eq!(c.classify_path(Path::new(".hidden")), Category::Archivos);
    }

    #[test]
    fn design_and_3d_files_are_not_editable() {
        let c = Classifier::default();
        for e in ["psd", "ai", "c4d", "blend", "drp", "indd", "psb"] {
            assert_eq!(c.classify_ext(e), Category::Archivos, "{e}");
        }
    }

    #[test]
    fn custom_rules_cannot_break_editable_exclusivity() {
        let rules = vec![
            CustomRule { extension: "psd".into(), category: Category::Editable },
            CustomRule { extension: "aep".into(), category: Category::Archivos },
            CustomRule { extension: ".BRAW".into(), category: Category::Archivos },
            CustomRule { extension: "xmp".into(), category: Category::Imagen },
        ];
        let c = Classifier::new(&rules);
        assert_eq!(c.classify_ext("psd"), Category::Archivos);
        assert_eq!(c.classify_ext("aep"), Category::Editable);
        assert_eq!(c.classify_ext("braw"), Category::Archivos);
        assert_eq!(c.classify_ext("XMP"), Category::Imagen);
        assert_eq!(validate_rule(&rules[0]), Err(RuleRejection::EditableIsReserved));
        assert_eq!(validate_rule(&rules[1]), Err(RuleRejection::ProtectedExtension));
    }

    #[test]
    fn category_allowed_enforces_exclusivity() {
        let c = Classifier::default();
        assert!(c.category_allowed("aep", Category::Editable));
        assert!(!c.category_allowed("aep", Category::Archivos));
        assert!(!c.category_allowed("psd", Category::Editable));
        assert!(c.category_allowed("psd", Category::Imagen));
    }
}
