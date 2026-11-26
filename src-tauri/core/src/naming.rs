//! Nomenclatura de proyectos: `DD.MM.AA CLIENTE - NOMBRE DEL PROYECTO`.

use crate::paths::sanitize_component;
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

pub const DEFAULT_TEMPLATE: &str = "{DD}.{MM}.{AA} {CLIENTE} - {PROYECTO}";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct NamingSettings {
    /// Plantilla con los tokens {DD} {MM} {AA} {AAAA} {CLIENTE} {PROYECTO}.
    pub template: String,
    /// Convertir cliente y proyecto a mayúsculas.
    pub uppercase: bool,
}

impl Default for NamingSettings {
    fn default() -> Self {
        NamingSettings {
            template: DEFAULT_TEMPLATE.into(),
            uppercase: true,
        }
    }
}

fn clean_text(s: &str, uppercase: bool) -> String {
    let collapsed = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let collapsed = if uppercase { collapsed.to_uppercase() } else { collapsed };
    collapsed
}

/// Genera el nombre de la carpeta del proyecto. El resultado es un nombre de
/// carpeta válido en Windows y macOS.
pub fn format_project_name(settings: &NamingSettings, date: NaiveDate, client: &str, project: &str) -> String {
    let template = if settings.template.trim().is_empty() {
        DEFAULT_TEMPLATE
    } else {
        settings.template.as_str()
    };
    let client = clean_text(client, settings.uppercase);
    let project = clean_text(project, settings.uppercase);
    let values = [
        ("{DD}", format!("{:02}", date.day())),
        ("{MM}", format!("{:02}", date.month())),
        ("{AAAA}", format!("{:04}", date.year())),
        ("{AA}", format!("{:02}", date.year().rem_euclid(100))),
        ("{CLIENTE}", client),
        ("{PROYECTO}", project),
    ];
    // Separar la plantilla en literales y tokens.
    enum Part {
        Lit(String),
        Val(String),
    }
    let mut parts: Vec<Part> = Vec::new();
    let mut rest = template;
    while !rest.is_empty() {
        let next = values
            .iter()
            .filter_map(|(t, v)| rest.find(t).map(|i| (i, *t, v)))
            .min_by_key(|(i, t, _)| (*i, std::cmp::Reverse(t.len())));
        match next {
            Some((i, t, v)) => {
                if i > 0 {
                    parts.push(Part::Lit(rest[..i].to_string()));
                }
                parts.push(Part::Val(v.clone()));
                rest = &rest[i + t.len()..];
            }
            None => {
                parts.push(Part::Lit(rest.to_string()));
                rest = "";
            }
        }
    }
    // Un valor vacío arrastra el separador contiguo (« - », «_»...) para no dejarlo huérfano.
    let is_sep = |c: char| c == '-' || c == '_' || c == '–' || c == '—' || c.is_whitespace();
    for i in 0..parts.len() {
        let empty = matches!(&parts[i], Part::Val(v) if v.is_empty());
        if !empty {
            continue;
        }
        if let Some(Part::Lit(l)) = parts.get_mut(i + 1) {
            let trimmed = l.trim_start_matches(is_sep).to_string();
            *l = trimmed;
        } else if i > 0 {
            if let Some(Part::Lit(l)) = parts.get_mut(i - 1) {
                let trimmed = l.trim_end_matches(is_sep).to_string();
                *l = trimmed;
            }
        }
    }
    let joined: String = parts
        .into_iter()
        .map(|p| match p {
            Part::Lit(s) | Part::Val(s) => s,
        })
        .collect();
    let out = joined.split_whitespace().collect::<Vec<_>>().join(" ");
    let out = out.trim_matches(|c: char| is_sep(c)).to_string();
    sanitize_component(&out)
}

/// Fecha en formato ISO `AAAA-MM-DD` (lo que envía la interfaz).
pub fn parse_iso_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ParsedProjectName {
    pub date: String,
    pub client: String,
    pub project: String,
}

/// Intenta leer una carpeta que ya sigue la nomenclatura `DD.MM.AA CLIENTE - PROYECTO`.
pub fn parse_project_name(name: &str) -> Option<ParsedProjectName> {
    let name = name.trim();
    let (date_part, rest) = name.split_once(' ')?;
    let parts: Vec<&str> = date_part.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let d: u32 = parts[0].parse().ok()?;
    let m: u32 = parts[1].parse().ok()?;
    let y: i32 = parts[2].parse().ok()?;
    let year = if parts[2].len() == 2 { 2000 + y } else { y };
    let date = NaiveDate::from_ymd_opt(year, m, d)?;
    let (client, project) = rest.split_once(" - ")?;
    Some(ParsedProjectName {
        date: date.format("%Y-%m-%d").to_string(),
        client: client.trim().to_string(),
        project: project.trim().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, dd: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, dd).unwrap()
    }

    #[test]
    fn formats_the_documented_example() {
        let s = NamingSettings::default();
        assert_eq!(
            format_project_name(&s, d(2026, 11, 25), "Banco Popular", "Campaña Navidad"),
            "25.11.26 BANCO POPULAR - CAMPAÑA NAVIDAD"
        );
    }

    #[test]
    fn collapses_spaces_and_strips_invalid_chars() {
        let s = NamingSettings::default();
        assert_eq!(
            format_project_name(&s, d(2026, 1, 5), "  Acme   S.A. ", "Spot: 30\"/TV?"),
            "05.01.26 ACME S.A. - SPOT_ 30__TV_"
        );
    }

    #[test]
    fn respects_uppercase_setting_and_custom_template() {
        let s = NamingSettings {
            template: "{AAAA}-{MM}-{DD}_{CLIENTE}_{PROYECTO}".into(),
            uppercase: false,
        };
        assert_eq!(format_project_name(&s, d(2026, 3, 9), "Nike", "Air"), "2026-03-09_Nike_Air");
    }

    #[test]
    fn handles_missing_project() {
        let s = NamingSettings::default();
        assert_eq!(format_project_name(&s, d(2026, 11, 25), "Cliente", ""), "25.11.26 CLIENTE");
    }

    #[test]
    fn handles_missing_client_without_orphan_separator() {
        let s = NamingSettings::default();
        assert_eq!(format_project_name(&s, d(2026, 11, 25), "", "Backup Navidad"), "25.11.26 BACKUP NAVIDAD");
        let s = NamingSettings { template: "{CLIENTE}_{PROYECTO}_{AAAA}".into(), uppercase: false };
        assert_eq!(format_project_name(&s, d(2026, 1, 2), "", "X"), "X_2026");
    }

    #[test]
    fn parses_existing_names() {
        let p = parse_project_name("25.11.26 BANCO POPULAR - CAMPAÑA NAVIDAD").unwrap();
        assert_eq!(p.date, "2026-11-25");
        assert_eq!(p.client, "BANCO POPULAR");
        assert_eq!(p.project, "CAMPAÑA NAVIDAD");
        assert!(parse_project_name("Proyecto suelto").is_none());
        assert!(parse_project_name("45.13.26 A - B").is_none());
    }
}
