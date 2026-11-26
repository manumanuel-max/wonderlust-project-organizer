//! Preferencias locales (JSON en la carpeta de configuración de la aplicación).

use crate::classify::{validate_rule, CustomRule, RuleRejection};
use crate::copy::CopySettings;
use crate::duplicates::{DefaultConflictAction, DuplicatePolicy};
use crate::error::{CoreError, CoreResult};
use crate::exclusions::ExclusionSettings;
use crate::naming::NamingSettings;
use crate::plan::{OrganizeMode, PlanOptions};
use crate::sequence::SequenceSettings;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct DuplicateSettings {
    pub detect_content: bool,
    pub policy: DuplicatePolicy,
    pub default_conflict_action: DefaultConflictAction,
}

impl Default for DuplicateSettings {
    fn default() -> Self {
        DuplicateSettings { detect_content: true, policy: DuplicatePolicy::CopyAll, default_conflict_action: DefaultConflictAction::Ask }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FrequentPath {
    pub label: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    /// `dark`, `light` o `system`.
    pub theme: String,
    pub naming: NamingSettings,
    pub custom_rules: Vec<CustomRule>,
    pub exclusions: ExclusionSettings,
    pub sequences: SequenceSettings,
    pub render_patterns: Vec<String>,
    pub always_create_render_folder: bool,
    pub preserve_subfolders: bool,
    pub default_mode: OrganizeMode,
    pub duplicates: DuplicateSettings,
    pub copy: CopySettings,
    pub frequent_paths: Vec<FrequentPath>,
    pub recent_destinations: Vec<String>,
    /// Guardar reporte y mapa de rutas en `<proyecto>/_wonderlust_reportes/`.
    pub write_report_to_destination: bool,
    /// Al mover entre discos: qué hacer con el original verificado.
    pub move_removal: crate::copy::RemovalMode,
    /// Opción "Organizar con Wonderlust" en el menú contextual de Finder / Explorador.
    pub context_menu: bool,
    /// Sonido al terminar una copia o un movimiento.
    pub finish_sound: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            theme: "dark".into(),
            naming: NamingSettings::default(),
            custom_rules: Vec::new(),
            exclusions: ExclusionSettings::default(),
            sequences: SequenceSettings::default(),
            render_patterns: crate::folders::default_render_patterns(),
            always_create_render_folder: false,
            preserve_subfolders: true,
            default_mode: OrganizeMode::Organize,
            duplicates: DuplicateSettings::default(),
            copy: CopySettings::default(),
            frequent_paths: Vec::new(),
            recent_destinations: Vec::new(),
            write_report_to_destination: true,
            move_removal: crate::copy::RemovalMode::Trash,
            context_menu: true,
            finish_sound: true,
        }
    }
}

impl AppSettings {
    pub fn plan_options(&self) -> PlanOptions {
        PlanOptions {
            mode: self.default_mode,
            preserve_subfolders: self.preserve_subfolders,
            exclusions: self.exclusions.clone(),
            sequences: self.sequences.clone(),
            render_patterns: self.render_patterns.clone(),
            always_create_render_folder: self.always_create_render_folder,
            custom_rules: self.custom_rules.clone(),
        }
    }

    /// Valida y normaliza antes de guardar.
    pub fn validate(&mut self) -> CoreResult<()> {
        for r in &self.custom_rules {
            if let Err(e) = validate_rule(r) {
                let msg = match e {
                    RuleRejection::EditableIsReserved => format!(
                        "La regla «.{}» no puede apuntar a «4. editable»: esa carpeta es exclusiva de AEP, AEPX y PRPROJ",
                        r.extension
                    ),
                    RuleRejection::ProtectedExtension => {
                        format!("«.{}» siempre se clasifica como editable y no se puede reasignar", r.extension)
                    }
                    RuleRejection::EmptyExtension => "Hay una regla sin extensión".to_string(),
                };
                return Err(CoreError::Invalid(msg));
            }
        }
        for r in self.custom_rules.iter_mut() {
            r.extension = crate::classify::normalize_ext(&r.extension);
        }
        self.copy.concurrency = self.copy.concurrency.clamp(1, 8);
        self.copy.retries = self.copy.retries.min(10);
        self.copy.buffer_kb = self.copy.buffer_kb.clamp(256, 32768);
        self.sequences.min_frames = self.sequences.min_frames.clamp(2, 1000);
        self.sequences.min_density = self.sequences.min_density.clamp(0.1, 1.0);
        self.recent_destinations.truncate(8);
        if !["dark", "light", "system"].contains(&self.theme.as_str()) {
            self.theme = "dark".into();
        }
        Ok(())
    }

    pub fn remember_destination(&mut self, path: &str) {
        self.recent_destinations.retain(|p| p != path);
        self.recent_destinations.insert(0, path.to_string());
        self.recent_destinations.truncate(8);
    }
}

/// Lee un JSON; si está dañado, se conserva una copia `.danado` y se usan valores por defecto.
pub fn load_json<T: serde::de::DeserializeOwned + Default>(path: &Path) -> T {
    match std::fs::read_to_string(path) {
        Ok(s) => match serde_json::from_str(&s) {
            Ok(v) => v,
            Err(_) => {
                let _ = std::fs::copy(path, path.with_extension("json.danado"));
                T::default()
            }
        },
        Err(_) => T::default(),
    }
}

/// Escritura atómica de archivos propios de la aplicación.
pub fn save_json<T: Serialize>(path: &Path, value: &T) -> CoreResult<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| CoreError::io(p, e))?;
    }
    let tmp = path.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(value)?;
    std::fs::write(&tmp, data).map_err(|e| CoreError::io(&tmp, e))?;
    std::fs::rename(&tmp, path).map_err(|e| CoreError::io(path, e))
}

pub fn load(path: &Path) -> AppSettings {
    load_json(path)
}

pub fn save(path: &Path, settings: &AppSettings) -> CoreResult<()> {
    let mut s = settings.clone();
    s.validate()?;
    save_json(path, &s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Category;

    #[test]
    fn roundtrip_and_defaults_for_missing_fields() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("settings.json");
        assert_eq!(load(&p), AppSettings::default());
        let mut s = AppSettings::default();
        s.theme = "light".into();
        s.custom_rules.push(CustomRule { extension: ".XMP".into(), category: Category::Imagen });
        save(&p, &s).unwrap();
        let back = load(&p);
        assert_eq!(back.theme, "light");
        assert_eq!(back.custom_rules[0].extension, "xmp");
        std::fs::write(&p, r#"{"theme":"light"}"#).unwrap();
        let partial = load(&p);
        assert_eq!(partial.theme, "light");
        assert_eq!(partial.copy, CopySettings::default());
    }

    #[test]
    fn rejects_rules_that_touch_editable() {
        let d = tempfile::tempdir().unwrap();
        let mut s = AppSettings::default();
        s.custom_rules.push(CustomRule { extension: "psd".into(), category: Category::Editable });
        assert!(save(&d.path().join("s.json"), &s).is_err());
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("settings.json");
        std::fs::write(&p, "{no es json").unwrap();
        assert_eq!(load(&p), AppSettings::default());
        assert!(d.path().join("settings.json.danado").exists());
    }
}
