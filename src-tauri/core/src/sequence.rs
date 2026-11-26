//! Detección de secuencias de imágenes (animación, composición, renders).
//!
//! Los fotogramas NUNCA se renombran: esta etapa solo agrupa.

use crate::paths::compare_key;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct SequenceSettings {
    pub enabled: bool,
    /// Número mínimo de fotogramas consecutivos para aceptar una secuencia sin preguntar.
    pub min_frames: usize,
    /// Proporción mínima de fotogramas presentes en el rango (0–1).
    pub min_density: f64,
    /// Prefijos típicos de fotos de cámara: se tratan como casos ambiguos.
    pub camera_prefixes: Vec<String>,
}

impl Default for SequenceSettings {
    fn default() -> Self {
        SequenceSettings {
            enabled: true,
            min_frames: 5,
            min_density: 0.9,
            camera_prefixes: [
                "IMG_", "IMG", "DSC", "_DSC", "DSCF", "DSCN", "_MG_", "MVI_", "GOPR", "GH0", "GX0", "DJI_",
                "PXL_", "P10", "_1", "R0",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        }
    }
}

/// Formatos de fotografía de cámara (RAW/JPEG/HEIC).
const PHOTO_EXTS: &[&str] = &["jpg", "jpeg", "heic", "cr2", "cr3", "nef", "arw", "dng"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SequenceConfidence {
    /// Secuencia clara: se agrupa automáticamente.
    High,
    /// Requiere confirmación del usuario.
    Ambiguous,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SequenceInfo {
    /// Clave estable: `carpeta|prefijo|extensión`.
    pub key: String,
    pub rel_dir: String,
    pub prefix: String,
    pub ext: String,
    /// Dígitos de relleno (0 si la numeración no tiene relleno uniforme).
    pub padding: usize,
    pub first: u64,
    pub last: u64,
    pub count: usize,
    pub missing_count: u64,
    /// Rangos faltantes (inclusive), limitados a los primeros 200.
    pub missing_ranges: Vec<(u64, u64)>,
    pub confidence: SequenceConfidence,
    /// Motivo de la ambigüedad, si la hay.
    pub ambiguity: Option<String>,
    /// Nombre sugerido para la subcarpeta de la secuencia.
    pub folder_name: String,
    /// `true` si la secuencia ya está sola en su propia carpeta.
    pub owns_folder: bool,
    pub file_ids: Vec<u32>,
    pub total_size: u64,
}

/// Archivo candidato (solo imágenes).
#[derive(Debug, Clone)]
pub struct Candidate<'a> {
    pub id: u32,
    pub rel_dir: &'a str,
    pub name: &'a str,
    pub size: u64,
}

/// Separa `Explosion_0001.png` en (`Explosion_`, `0001`, `png`).
pub fn split_frame_name(name: &str) -> Option<(String, String, String)> {
    let dot = name.rfind('.')?;
    if dot == 0 {
        return None;
    }
    let stem = &name[..dot];
    let ext = name[dot + 1..].to_lowercase();
    let digits_start = stem
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_digit())
        .last()
        .map(|(i, _)| i)?;
    let digits = &stem[digits_start..];
    if digits.is_empty() || digits.len() > 9 {
        return None;
    }
    Some((stem[..digits_start].to_string(), digits.to_string(), ext))
}

fn clean_prefix(prefix: &str) -> String {
    prefix
        .trim_end_matches(|c: char| c == '_' || c == '-' || c == '.' || c.is_whitespace())
        .to_string()
}

fn is_camera_like(prefix: &str, ext: &str, settings: &SequenceSettings) -> bool {
    if !PHOTO_EXTS.contains(&ext) {
        return false;
    }
    let p = prefix.to_uppercase();
    settings
        .camera_prefixes
        .iter()
        .any(|cp| !cp.is_empty() && p.starts_with(&cp.to_uppercase()))
}

/// Detecta secuencias.
///
/// `dir_file_counts` es el número de archivos (no ocultos) por carpeta relativa;
/// sirve para saber si la secuencia ya tiene su carpeta propia.
pub fn detect_sequences(
    candidates: &[Candidate],
    dir_file_counts: &HashMap<String, usize>,
    settings: &SequenceSettings,
) -> Vec<SequenceInfo> {
    if !settings.enabled {
        return Vec::new();
    }
    // Agrupar por carpeta + prefijo + extensión.
    let mut groups: BTreeMap<String, Vec<(u64, &str, u32, u64, String)>> = BTreeMap::new();
    let mut meta: HashMap<String, (String, String, String)> = HashMap::new();
    for c in candidates {
        let Some((prefix, digits, ext)) = split_frame_name(c.name) else { continue };
        let Ok(n) = digits.parse::<u64>() else { continue };
        let key = format!("{}|{}|{}", c.rel_dir, compare_key(&prefix), ext);
        meta.entry(key.clone())
            .or_insert_with(|| (c.rel_dir.to_string(), prefix.clone(), ext.clone()));
        groups.entry(key).or_default().push((n, c.name, c.id, c.size, digits));
    }

    let mut out = Vec::new();
    for (key, mut frames) in groups {
        if frames.len() < 2 {
            continue;
        }
        frames.sort_by_key(|f| f.0);
        let (rel_dir, prefix, ext) = meta.remove(&key).unwrap();
        let first = frames.first().unwrap().0;
        let last = frames.last().unwrap().0;
        let mut numbers: Vec<u64> = frames.iter().map(|f| f.0).collect();
        numbers.dedup();
        let count = frames.len();
        let range = last - first + 1;
        let density = numbers.len() as f64 / range as f64;

        let mut missing_ranges = Vec::new();
        let mut missing_count = 0u64;
        for w in numbers.windows(2) {
            if w[1] > w[0] + 1 {
                missing_count += w[1] - w[0] - 1;
                if missing_ranges.len() < 200 {
                    missing_ranges.push((w[0] + 1, w[1] - 1));
                }
            }
        }

        let lens: Vec<usize> = frames.iter().map(|f| f.4.len()).collect();
        let padding = if lens.iter().all(|l| *l == lens[0]) { lens[0] } else { 0 };

        let camera = is_camera_like(&prefix, &ext, settings);
        let (confidence, ambiguity) = if camera {
            (
                SequenceConfidence::Ambiguous,
                Some("La numeración parece de fotografías de cámara, no de una secuencia".to_string()),
            )
        } else if count >= settings.min_frames && density >= settings.min_density {
            (SequenceConfidence::High, None)
        } else if count < settings.min_frames && density >= settings.min_density {
            (
                SequenceConfidence::Ambiguous,
                Some(format!("Solo {count} archivos numerados consecutivos")),
            )
        } else if density >= 0.2 {
            (
                SequenceConfidence::Ambiguous,
                Some(format!(
                    "Numeración con muchos huecos ({} faltantes de {})",
                    missing_count, range
                )),
            )
        } else {
            // Numeración dispersa: imágenes independientes.
            continue;
        };

        let dir_total = dir_file_counts.get(&rel_dir).copied().unwrap_or(usize::MAX);
        let owns_folder = !rel_dir.is_empty() && dir_total == count;
        let folder_name = if owns_folder {
            rel_dir.rsplit('/').next().unwrap_or(&rel_dir).to_string()
        } else {
            let p = clean_prefix(&prefix);
            if p.is_empty() {
                format!("Secuencia {}", ext.to_uppercase())
            } else {
                p
            }
        };
        out.push(SequenceInfo {
            key,
            rel_dir,
            prefix,
            ext,
            padding,
            first,
            last,
            count,
            missing_count,
            missing_ranges,
            confidence,
            ambiguity,
            folder_name,
            owns_folder,
            total_size: frames.iter().map(|f| f.3).sum(),
            file_ids: frames.iter().map(|f| f.2).collect(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cands(dir: &'static str, names: &[String]) -> Vec<(String, String)> {
        names.iter().map(|n| (dir.to_string(), n.clone())).collect()
    }

    fn run(files: &[(String, String)], counts: &[(&str, usize)]) -> Vec<SequenceInfo> {
        let c: Vec<Candidate> = files
            .iter()
            .enumerate()
            .map(|(i, (d, n))| Candidate { id: i as u32, rel_dir: d, name: n, size: 10 })
            .collect();
        let counts = counts.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        detect_sequences(&c, &counts, &SequenceSettings::default())
    }

    #[test]
    fn splits_names() {
        assert_eq!(
            split_frame_name("Explosion_0001.png"),
            Some(("Explosion_".into(), "0001".into(), "png".into()))
        );
        assert_eq!(split_frame_name("shot.0100.EXR"), Some(("shot.".into(), "0100".into(), "exr".into())));
        assert_eq!(split_frame_name("logo.png"), None);
        assert_eq!(split_frame_name("frame_1_v2b.png"), None);
    }

    #[test]
    fn detects_the_documented_example() {
        let names: Vec<String> = (1..=300).map(|i| format!("Explosion_{i:04}.png")).collect();
        let s = run(&cands("renders", &names), &[("renders", 300)]);
        assert_eq!(s.len(), 1);
        let s = &s[0];
        assert_eq!(s.count, 300);
        assert_eq!((s.first, s.last, s.padding), (1, 300, 4));
        assert_eq!(s.missing_count, 0);
        assert_eq!(s.confidence, SequenceConfidence::High);
        assert!(s.owns_folder);
        assert_eq!(s.folder_name, "renders");
    }

    #[test]
    fn reports_missing_frames_in_incomplete_sequences() {
        let names: Vec<String> = (1..=100)
            .filter(|i| !(40..=42).contains(i) && *i != 77)
            .map(|i| format!("Humo_{i:03}.exr"))
            .collect();
        let s = &run(&cands("", &names), &[("", 200)])[0];
        assert_eq!(s.missing_count, 4);
        assert_eq!(s.missing_ranges, vec![(40, 42), (77, 77)]);
        assert_eq!(s.confidence, SequenceConfidence::High);
        assert_eq!(s.folder_name, "Humo");
        assert!(!s.owns_folder);
    }

    #[test]
    fn short_runs_are_ambiguous() {
        let names: Vec<String> = (1..=3).map(|i| format!("logo_{i}.png")).collect();
        let s = &run(&cands("", &names), &[])[0];
        assert_eq!(s.confidence, SequenceConfidence::Ambiguous);
    }

    #[test]
    fn camera_photos_are_ambiguous_not_sequences() {
        let names: Vec<String> = (1000..1050).map(|i| format!("IMG_{i}.JPG")).collect();
        let s = &run(&cands("DCIM", &names), &[])[0];
        assert_eq!(s.confidence, SequenceConfidence::Ambiguous);
    }

    #[test]
    fn sparse_numbering_is_not_a_sequence() {
        let names: Vec<String> = [1, 50, 300, 999].iter().map(|i| format!("foto_{i}.png")).collect();
        assert!(run(&cands("", &names), &[]).is_empty());
    }

    #[test]
    fn different_dirs_and_extensions_are_separate() {
        let mut files = Vec::new();
        for i in 1..=10 {
            files.push(("a".to_string(), format!("x_{i:03}.png")));
            files.push(("b".to_string(), format!("x_{i:03}.png")));
            files.push(("a".to_string(), format!("x_{i:03}.tif")));
        }
        assert_eq!(run(&files, &[]).len(), 3);
    }

    #[test]
    fn unpadded_numbering() {
        let names: Vec<String> = (1..=12).map(|i| format!("anim{i}.tga")).collect();
        let s = &run(&cands("", &names), &[])[0];
        assert_eq!(s.padding, 0);
        assert_eq!(s.count, 12);
        assert_eq!(s.folder_name, "anim");
    }
}
