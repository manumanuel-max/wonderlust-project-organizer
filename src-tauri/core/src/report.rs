//! Reporte final y exportaciones (CSV, JSON, TXT y mapa de rutas).

use crate::copy::{JobManifest, JobOutcome, OpResult, VerifyMode};
use crate::duplicates::{ExcludedEntry, OpAction};
use crate::paths::join_rel;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReportStatus {
    /// Todo copiado y verificado.
    Completed,
    /// Terminó, pero hay archivos con errores.
    CompletedWithErrors,
    /// Cancelado por el usuario: quedan archivos pendientes.
    Cancelled,
    /// Detenido por un error grave (p. ej. sin espacio).
    Aborted,
}

impl ReportStatus {
    pub fn label(self) -> &'static str {
        match self {
            ReportStatus::Completed => "Completado y verificado",
            ReportStatus::CompletedWithErrors => "Finalizado con errores",
            ReportStatus::Cancelled => "Cancelado (incompleto)",
            ReportStatus::Aborted => "Detenido por error (incompleto)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportEntry {
    pub idx: u32,
    pub source: String,
    pub dest: String,
    pub dest_rel: String,
    pub size: u64,
    pub status: String,
    pub detail: String,
    pub sha256: String,
    pub renamed: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportTotals {
    pub planned: usize,
    pub copied: usize,
    pub verified: usize,
    pub skipped: usize,
    pub duplicates: usize,
    pub excluded: usize,
    pub failed: usize,
    pub pending: usize,
    pub bytes_copied: u64,
    pub bytes_transferred_this_run: u64,
    pub bytes_excluded: u64,
    pub bytes_failed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobReport {
    pub job_id: String,
    pub kind: String,
    pub client: String,
    pub project: String,
    pub dest_root: PathBuf,
    pub sources: Vec<PathBuf>,
    pub created_at: String,
    pub finished_at: String,
    pub duration_ms: u64,
    pub status: ReportStatus,
    pub status_label: String,
    pub success: bool,
    pub integrity_mode: VerifyMode,
    pub integrity_message: String,
    pub totals: ReportTotals,
    pub aborted_reason: Option<String>,
    pub entries: Vec<ReportEntry>,
    pub excluded: Vec<ExcludedEntry>,
    pub warnings: Vec<String>,
}

pub fn build_report(manifest: &JobManifest, outcome: &JobOutcome, finished_at: String) -> JobReport {
    let mut t = ReportTotals { excluded: manifest.excluded.len(), ..Default::default() };
    t.bytes_excluded = manifest.excluded.iter().map(|e| e.size).sum();
    t.bytes_transferred_this_run = outcome.bytes_copied;
    let mut entries = Vec::with_capacity(manifest.ops.len());
    for (op, r) in manifest.ops.iter().zip(&outcome.results) {
        if op.action == OpAction::Copy {
            t.planned += 1;
        }
        let (status, detail, sha) = match r {
            OpResult::Done { sha256, verified } => {
                t.copied += 1;
                t.bytes_copied += op.size;
                if *verified {
                    t.verified += 1;
                }
                ("copiado", if *verified { "Verificado SHA-256".to_string() } else { "Verificado por tamaño".to_string() }, sha256.clone())
            }
            OpResult::Skipped { reason } => {
                t.skipped += 1;
                if reason.contains("uplicado") || reason.contains("idéntico") {
                    t.duplicates += 1;
                }
                ("omitido", reason.clone(), String::new())
            }
            OpResult::Failed { error } => {
                t.failed += 1;
                t.bytes_failed += op.size;
                ("error", error.clone(), String::new())
            }
            OpResult::Pending => {
                t.pending += 1;
                ("pendiente", "No se completó".to_string(), String::new())
            }
        };
        entries.push(ReportEntry {
            idx: op.idx,
            source: op.src.display().to_string(),
            dest: join_rel(&manifest.dest_root, &op.dest_rel).display().to_string(),
            dest_rel: op.dest_rel.clone(),
            size: op.size,
            status: status.to_string(),
            detail,
            sha256: sha,
            renamed: op.renamed,
        });
    }
    let status = if outcome.aborted_reason.is_some() {
        ReportStatus::Aborted
    } else if outcome.cancelled || t.pending > 0 {
        ReportStatus::Cancelled
    } else if t.failed > 0 {
        ReportStatus::CompletedWithErrors
    } else {
        ReportStatus::Completed
    };
    let all_verified = match manifest.settings.verify {
        VerifyMode::Full => t.verified == t.copied,
        VerifyMode::Size => true,
    };
    let success = status == ReportStatus::Completed && all_verified && t.copied == t.planned;
    let integrity_message = if t.failed + t.pending > 0 {
        format!(
            "Backup NO completo: {} archivo(s) con error y {} pendiente(s). No se debe considerar un backup válido.",
            t.failed, t.pending
        )
    } else if manifest.settings.verify == VerifyMode::Full && all_verified {
        format!("Los {} archivos copiados se verificaron con SHA-256 contra el origen.", t.copied)
    } else if manifest.settings.verify == VerifyMode::Full {
        format!("{} de {} archivos verificados con SHA-256.", t.verified, t.copied)
    } else {
        "Verificación rápida: solo se comprobó el tamaño de los archivos.".to_string()
    };
    JobReport {
        job_id: manifest.job_id.clone(),
        kind: manifest.kind.clone(),
        client: manifest.client.clone(),
        project: manifest.project.clone(),
        dest_root: manifest.dest_root.clone(),
        sources: manifest.sources.clone(),
        created_at: manifest.created_at.clone(),
        finished_at,
        duration_ms: outcome.duration_ms,
        status,
        status_label: status.label().to_string(),
        success,
        integrity_mode: manifest.settings.verify,
        integrity_message,
        totals: t,
        aborted_reason: outcome.aborted_reason.clone(),
        entries,
        excluded: manifest.excluded.clone(),
        warnings: manifest.warnings.clone(),
    }
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.2} {}", UNITS[u]).replace('.', ",")
    }
}

pub fn human_duration(ms: u64) -> String {
    let s = ms / 1000;
    let (h, m, s) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h} h {m:02} min {s:02} s")
    } else if m > 0 {
        format!("{m} min {s:02} s")
    } else {
        format!("{s} s")
    }
}

fn csv_field(s: &str) -> String {
    if s.contains(';') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

const BOM: &str = "\u{FEFF}";

/// CSV separado por `;` con BOM UTF-8 (se abre correctamente en Excel en español).
pub fn to_csv(r: &JobReport) -> String {
    let mut out = String::from(BOM);
    out.push_str("estado;origen;destino;tamaño_bytes;detalle;sha256\n");
    for e in &r.entries {
        out.push_str(&[
            csv_field(&e.status),
            csv_field(&e.source),
            csv_field(&e.dest),
            e.size.to_string(),
            csv_field(&e.detail),
            e.sha256.clone(),
        ]
        .join(";"));
        out.push('\n');
    }
    for e in &r.excluded {
        out.push_str(&["excluido".to_string(), csv_field(&e.rel_src), String::new(), e.size.to_string(), csv_field(&e.reason), String::new()].join(";"));
        out.push('\n');
    }
    out
}

/// Mapa de rutas originales → nuevas (útil para revincular en Adobe).
pub fn to_path_map(r: &JobReport) -> String {
    let mut out = String::from(BOM);
    out.push_str("ruta_original;ruta_nueva;estado\n");
    for e in &r.entries {
        out.push_str(&format!("{};{};{}\n", csv_field(&e.source), csv_field(&e.dest), csv_field(&e.status)));
    }
    out
}

pub fn to_json(r: &JobReport) -> String {
    serde_json::to_string_pretty(r).unwrap_or_default()
}

pub fn to_txt(r: &JobReport) -> String {
    let t = &r.totals;
    let mut s = String::new();
    s.push_str("WONDERLUST PROJECT ORGANIZER — REPORTE DE OPERACIÓN\n");
    s.push_str(&"=".repeat(60));
    s.push('\n');
    s.push_str(&format!("Operación:   {}\n", r.kind));
    if !r.client.is_empty() || !r.project.is_empty() {
        s.push_str(&format!("Cliente:     {}\nProyecto:    {}\n", r.client, r.project));
    }
    s.push_str(&format!("Destino:     {}\n", r.dest_root.display()));
    for src in &r.sources {
        s.push_str(&format!("Origen:      {}\n", src.display()));
    }
    s.push_str(&format!("Inicio:      {}\nFin:         {}\nDuración:    {}\n\n", r.created_at, r.finished_at, human_duration(r.duration_ms)));
    s.push_str(&format!("ESTADO: {}\n", r.status_label.to_uppercase()));
    s.push_str(&format!("Integridad: {}\n", r.integrity_message));
    if let Some(reason) = &r.aborted_reason {
        s.push_str(&format!("Motivo de la detención: {reason}\n"));
    }
    s.push('\n');
    s.push_str(&format!("Archivos programados:   {}\n", t.planned));
    s.push_str(&format!("Copiados correctamente: {} ({})\n", t.copied, human_size(t.bytes_copied)));
    s.push_str(&format!("Verificados SHA-256:    {}\n", t.verified));
    s.push_str(&format!("Omitidos:               {} (duplicados: {})\n", t.skipped, t.duplicates));
    s.push_str(&format!("Excluidos:              {} ({})\n", t.excluded, human_size(t.bytes_excluded)));
    s.push_str(&format!("Con errores:            {}\n", t.failed));
    s.push_str(&format!("Pendientes:             {}\n", t.pending));
    let failed: Vec<&ReportEntry> = r.entries.iter().filter(|e| e.status == "error" || e.status == "pendiente").collect();
    if !failed.is_empty() {
        s.push_str("\nARCHIVOS CON ERRORES O PENDIENTES\n");
        for e in failed {
            s.push_str(&format!("  - {}\n    {}\n", e.source, e.detail));
        }
    }
    let skipped: Vec<&ReportEntry> = r.entries.iter().filter(|e| e.status == "omitido").collect();
    if !skipped.is_empty() {
        s.push_str("\nARCHIVOS OMITIDOS\n");
        for e in skipped {
            s.push_str(&format!("  - {} — {}\n", e.source, e.detail));
        }
    }
    let renamed: Vec<&ReportEntry> = r.entries.iter().filter(|e| e.renamed && e.status == "copiado").collect();
    if !renamed.is_empty() {
        s.push_str("\nCOPIAS RENOMBRADAS (el original no se modificó)\n");
        for e in renamed {
            s.push_str(&format!("  - {} → {}\n", e.source, e.dest_rel));
        }
    }
    if !r.excluded.is_empty() {
        s.push_str("\nEXCLUIDOS\n");
        for e in &r.excluded {
            s.push_str(&format!("  - {} — {}\n", e.rel_src, e.reason));
        }
    }
    if !r.warnings.is_empty() {
        s.push_str("\nADVERTENCIAS\n");
        for w in &r.warnings {
            s.push_str(&format!("  - {w}\n"));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_and_durations() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1536), "1,50 KB");
        assert_eq!(human_size(5 * 1024 * 1024 * 1024), "5,00 GB");
        assert_eq!(human_duration(3_723_000), "1 h 02 min 03 s");
        assert_eq!(human_duration(65_000), "1 min 05 s");
    }

    #[test]
    fn csv_escaping() {
        assert_eq!(csv_field("a;b"), "\"a;b\"");
        assert_eq!(csv_field("di \"x\""), "\"di \"\"x\"\"\"");
        assert_eq!(csv_field("ok"), "ok");
    }
}
