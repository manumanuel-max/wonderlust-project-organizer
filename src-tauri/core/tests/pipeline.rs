//! Pruebas de integración de extremo a extremo en directorios temporales.
//! Nunca tocan archivos reales del usuario.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use wpo_core::copy::{run_job, CopySettings, JobControl, JobManifest, Journal, OpResult, VerifyMode};
use wpo_core::duplicates::{analyze, build_operations, DefaultConflictAction, DuplicatePolicy, OpAction, Resolution};
use wpo_core::plan::{build_plan, OrganizeMode, Overrides, PlanOptions};
use wpo_core::preflight::run_preflight;
use wpo_core::report::{build_report, to_csv, to_json, to_path_map, to_txt, ReportStatus};
use wpo_core::scan::scan;

fn write(p: &Path, data: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, data).unwrap();
}

/// Huella del árbol de origen: ruta → (tamaño, fecha, sha256).
fn fingerprint(root: &Path) -> BTreeMap<PathBuf, (u64, SystemTime, String)> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            let m = std::fs::symlink_metadata(&p).unwrap();
            if m.is_dir() {
                stack.push(p);
            } else {
                let h = wpo_core::hashing::sha256_file(&p, &AtomicBool::new(false), |_| {}).unwrap();
                out.insert(p, (m.len(), m.modified().unwrap(), h));
            }
        }
    }
    out
}

fn pseudo_random(len: usize, seed: u64) -> Vec<u8> {
    let mut x = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x & 0xff) as u8
        })
        .collect()
}

fn sample_project(root: &Path) {
    write(&root.join("Footage/Día 1/A001_C001.MOV"), &pseudo_random(300_000, 1));
    write(&root.join("Footage/Día 1/A001_C002.mov"), &pseudo_random(250_000, 2));
    write(&root.join("Audio/Música/Tema principal.wav"), &pseudo_random(90_000, 3));
    write(&root.join("Audio/SFX/whoosh 01.wav"), &pseudo_random(10_000, 4));
    write(&root.join("Proyecto Campaña ñ.aep"), b"AEP fake project");
    write(&root.join("Edición final.prproj"), b"PRPROJ fake");
    write(&root.join("Diseño/Key visual.psd"), &pseudo_random(40_000, 5));
    write(&root.join("Docs/Guion v3 (aprobado).pdf"), b"%PDF fake");
    write(&root.join("Export/Spot 30s FINAL.mp4"), &pseudo_random(120_000, 6));
    for i in 1..=24 {
        write(&root.join(format!("Comp/Explosion/Explosion_{i:04}.png")), &pseudo_random(2_000, 100 + i));
    }
    write(&root.join("Adobe After Effects Auto-Save/Proyecto auto-save 1.aep"), b"autosave");
    write(&root.join(".DS_Store"), b"junk");
    write(&root.join("raro.xyz"), b"desconocido");
    std::fs::create_dir_all(root.join("Vacía")).unwrap();
}

struct Run {
    manifest: JobManifest,
    journal: PathBuf,
}

fn prepare(src: &Path, dest_root: &Path, journal_dir: &Path, opts: PlanOptions, verify: VerifyMode) -> Run {
    let inv = scan(&[src.to_path_buf()], &AtomicBool::new(false), |_, _| {}).unwrap();
    let plan = build_plan(&inv, &opts, &Overrides::default());
    let analysis = analyze(&plan, dest_root, true, DefaultConflictAction::KeepBoth, &AtomicBool::new(false), |_| {}).unwrap();
    let ops = build_operations(&plan, &analysis, &HashMap::new(), DuplicatePolicy::CopyAll, dest_root).unwrap();
    let pf = run_preflight(&ops, &[src.to_path_buf()], dest_root, &AtomicBool::new(false), |_, _| {});
    assert!(pf.ok, "{:?}", pf.issues);
    Run {
        manifest: JobManifest {
            job_id: "test".into(),
            created_at: "2026-11-25T10:00:00-05:00".into(),
            kind: "organize".into(),
            client: "BANCO POPULAR".into(),
            project: "CAMPAÑA NAVIDAD".into(),
            dest_root: dest_root.to_path_buf(),
            sources: vec![src.to_path_buf()],
            ops: ops.ops,
            dirs: ops.dirs,
            excluded: ops.excluded,
            settings: CopySettings { concurrency: 3, buffer_kb: 64, verify, ..Default::default() },
            warnings: vec![],
        },
        journal: journal_dir.join("journal.jsonl"),
    }
}

#[test]
fn full_organize_pipeline_preserves_originals() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("Proyecto original");
    sample_project(&src);
    let before = fingerprint(&src);
    let dest = d.path().join("Backups/25.11.26 BANCO POPULAR - CAMPAÑA NAVIDAD");
    std::fs::create_dir_all(dest.parent().unwrap()).unwrap();

    let run = prepare(&src, &dest, d.path(), PlanOptions::default(), VerifyMode::Full);
    let outcome = run_job(&run.manifest, &run.journal, &JobControl::default(), None, |_| {}).unwrap();
    let report = build_report(&run.manifest, &outcome, "fin".into());

    assert_eq!(report.status, ReportStatus::Completed, "{:#?}", report.entries.iter().filter(|e| e.status != "copiado").collect::<Vec<_>>());
    assert!(report.success);
    assert_eq!(report.totals.failed, 0);
    assert_eq!(report.totals.verified, report.totals.copied);
    assert_eq!(report.totals.excluded, 2); // autosave + .DS_Store

    // Estructura esperada.
    for f in ["1. imagen", "2. video", "3. audio", "4. editable", "5. archivos"] {
        assert!(dest.join(f).is_dir(), "{f}");
    }
    assert!(dest.join("2. video/Footage/Día 1/A001_C001.MOV").is_file());
    assert!(dest.join("2. video/Renders finales/Spot 30s FINAL.mp4").is_file());
    assert!(dest.join("3. audio/Música/Tema principal.wav").is_file());
    assert!(dest.join("3. audio/SFX/whoosh 01.wav").is_file());
    assert!(dest.join("4. editable/Proyecto Campaña ñ.aep").is_file());
    assert!(dest.join("4. editable/Edición final.prproj").is_file());
    assert!(dest.join("5. archivos/Diseño/Key visual.psd").is_file());
    assert!(dest.join("5. archivos/Docs/Guion v3 (aprobado).pdf").is_file());
    assert!(dest.join("5. archivos/raro.xyz").is_file());
    assert!(dest.join("1. imagen/Explosion/Explosion_0001.png").is_file());
    assert!(dest.join("1. imagen/Explosion/Explosion_0024.png").is_file());
    // Solo AEP/AEPX/PRPROJ en editable.
    for e in std::fs::read_dir(dest.join("4. editable")).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().to_lowercase();
        assert!(n.ends_with(".aep") || n.ends_with(".prproj"), "{n}");
    }
    // Contenido idéntico y fecha conservada.
    let a = std::fs::read(src.join("Footage/Día 1/A001_C001.MOV")).unwrap();
    let b = std::fs::read(dest.join("2. video/Footage/Día 1/A001_C001.MOV")).unwrap();
    assert_eq!(a, b);
    let ms = std::fs::metadata(src.join("Footage/Día 1/A001_C001.MOV")).unwrap().modified().unwrap();
    let md = std::fs::metadata(dest.join("2. video/Footage/Día 1/A001_C001.MOV")).unwrap().modified().unwrap();
    assert_eq!(
        ms.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
        md.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs()
    );
    // Sin temporales.
    assert_eq!(wpo_core::copy::cleanup_temp_files(&dest), 0);
    // Origen intacto (contenido, tamaño, fecha, nombres).
    assert_eq!(before, fingerprint(&src));

    // Exportaciones.
    assert!(to_csv(&report).contains("copiado"));
    assert!(to_json(&report).contains("\"success\": true"));
    assert!(to_txt(&report).contains("COMPLETADO Y VERIFICADO"));
    assert!(to_path_map(&report).contains("Renders finales"));
}

#[test]
fn preserve_hierarchy_backup() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("P");
    sample_project(&src);
    let dest = d.path().join("out/P");
    std::fs::create_dir_all(d.path().join("out")).unwrap();
    let mut opts = PlanOptions { mode: OrganizeMode::PreserveHierarchy, ..Default::default() };
    opts.exclusions.empty_folders = false;
    let run = prepare(&src, &dest, d.path(), opts, VerifyMode::Full);
    let outcome = run_job(&run.manifest, &run.journal, &JobControl::default(), None, |_| {}).unwrap();
    let report = build_report(&run.manifest, &outcome, "fin".into());
    assert!(report.success);
    assert!(dest.join("Footage/Día 1/A001_C001.MOV").is_file());
    assert!(dest.join("Proyecto Campaña ñ.aep").is_file());
    assert!(dest.join("Vacía").is_dir());
    assert!(!dest.join("2. video").exists());
}

#[test]
fn large_file_streaming_copy() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("src");
    // 48 MB en bloques: nunca se carga completo en memoria durante la copia.
    std::fs::create_dir_all(&src).unwrap();
    {
        use std::io::Write;
        let mut f = std::fs::File::create(src.join("master.mov")).unwrap();
        for i in 0..48 {
            f.write_all(&pseudo_random(1024 * 1024, i)).unwrap();
        }
    }
    let dest = d.path().join("P");
    let run = prepare(&src, &dest, d.path(), PlanOptions::default(), VerifyMode::Full);
    let progress_calls = std::sync::atomic::AtomicUsize::new(0);
    let outcome = run_job(&run.manifest, &run.journal, &JobControl::default(), None, |_| {
        progress_calls.fetch_add(1, Ordering::Relaxed);
    })
    .unwrap();
    let report = build_report(&run.manifest, &outcome, "fin".into());
    assert!(report.success);
    assert_eq!(report.totals.bytes_copied, 48 * 1024 * 1024);
    assert!(progress_calls.load(Ordering::Relaxed) >= 1);
    let a = wpo_core::hashing::sha256_file(&src.join("master.mov"), &AtomicBool::new(false), |_| {}).unwrap();
    let b = wpo_core::hashing::sha256_file(&dest.join("2. video/Renders finales/master.mov"), &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(a, b);
}

#[test]
fn cancel_then_resume_without_recopying_verified_files() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("src");
    for i in 0..40 {
        write(&src.join(format!("clip_{i:03}.mov")), &pseudo_random(400_000, i));
    }
    let dest = d.path().join("P");
    let mut run = prepare(&src, &dest, d.path(), PlanOptions { preserve_subfolders: false, ..Default::default() }, VerifyMode::Full);
    run.manifest.settings.concurrency = 1;
    run.manifest.settings.buffer_kb = 64;

    let control = Arc::new(JobControl::default());
    let c2 = control.clone();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(60));
        c2.cancel.store(true, Ordering::SeqCst);
    });
    let first = run_job(&run.manifest, &run.journal, &control, None, |_| {}).unwrap();
    canceller.join().unwrap();
    let r1 = build_report(&run.manifest, &first, "x".into());
    assert!(first.cancelled);
    assert_eq!(r1.status, ReportStatus::Cancelled);
    assert!(!r1.success, "un trabajo cancelado nunca se marca como exitoso");
    let done_first = r1.totals.copied;
    assert!(r1.totals.pending > 0);
    // No quedan temporales a medias ni archivos finales incompletos.
    assert_eq!(wpo_core::copy::cleanup_temp_files(&dest), 0);

    // Reanudar.
    let journal = Journal::load(&run.journal);
    assert_eq!(journal.values().filter(|r| matches!(r, OpResult::Done { .. })).count(), done_first);
    let second = run_job(&run.manifest, &run.journal, &JobControl::default(), None, |_| {}).unwrap();
    let r2 = build_report(&run.manifest, &second, "y".into());
    assert!(r2.success);
    assert_eq!(r2.totals.copied, 40);
    // Solo se transfirieron los pendientes.
    let pending_bytes: u64 = 400_000 * (40 - done_first as u64);
    assert_eq!(second.bytes_copied, pending_bytes);
}

#[test]
fn resume_after_crash_between_rename_and_journal() {
    // Simula un corte justo después del renombrado: el archivo final existe pero
    // el diario no lo registra. Debe reconocerse por hash, no fallar ni sobrescribir.
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("src");
    write(&src.join("a.wav"), &pseudo_random(5000, 1));
    write(&src.join("b.wav"), &pseudo_random(5000, 2));
    let dest = d.path().join("P");
    let run = prepare(&src, &dest, d.path(), PlanOptions::default(), VerifyMode::Full);
    write(&dest.join("3. audio/a.wav"), &std::fs::read(src.join("a.wav")).unwrap());
    write(&dest.join("3. audio/.b.wav.wlpart"), b"parcial");
    let out = run_job(&run.manifest, &run.journal, &JobControl::default(), None, |_| {}).unwrap();
    let r = build_report(&run.manifest, &out, "x".into());
    assert!(r.success, "{:#?}", r.entries);
    assert!(!dest.join("3. audio/.b.wav.wlpart").exists());
}

#[test]
fn never_overwrites_a_file_that_appears_in_destination() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("src");
    write(&src.join("a.wav"), b"nuevo contenido");
    let dest = d.path().join("P");
    let run = prepare(&src, &dest, d.path(), PlanOptions::default(), VerifyMode::Full);
    // Aparece un archivo distinto después del análisis.
    write(&dest.join("3. audio/a.wav"), b"otro");
    let out = run_job(&run.manifest, &run.journal, &JobControl::default(), None, |_| {}).unwrap();
    let r = build_report(&run.manifest, &out, "x".into());
    assert_eq!(r.totals.failed, 1);
    assert!(!r.success);
    assert_eq!(std::fs::read(dest.join("3. audio/a.wav")).unwrap(), b"otro");
}

#[test]
fn retry_only_failed_operations() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("src");
    write(&src.join("a.wav"), b"aaa");
    write(&src.join("b.wav"), b"bbb");
    let dest = d.path().join("P");
    let run = prepare(&src, &dest, d.path(), PlanOptions::default(), VerifyMode::Full);
    write(&dest.join("3. audio/b.wav"), b"bloqueo");
    let out = run_job(&run.manifest, &run.journal, &JobControl::default(), None, |_| {}).unwrap();
    assert_eq!(build_report(&run.manifest, &out, "x".into()).totals.failed, 1);
    let failed: HashSet<u32> = run
        .manifest
        .ops
        .iter()
        .zip(&out.results)
        .filter(|(_, r)| matches!(r, OpResult::Failed { .. }))
        .map(|(o, _)| o.idx)
        .collect();
    // El usuario resuelve el bloqueo moviendo su archivo (fuera de la app) y reintenta.
    std::fs::rename(dest.join("3. audio/b.wav"), d.path().join("b_aparte.wav")).unwrap();
    let out2 = run_job(&run.manifest, &run.journal, &JobControl::default(), Some(&failed), |_| {}).unwrap();
    let r = build_report(&run.manifest, &out2, "y".into());
    assert!(r.success);
    assert_eq!(out2.bytes_copied, 3);
}

#[test]
fn conflicts_with_existing_destination_and_keep_both() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("src");
    write(&src.join("clip.mov"), b"version nueva");
    write(&src.join("igual.mov"), b"mismo");
    let dest = d.path().join("P");
    write(&dest.join("2. video/clip.mov"), b"version vieja");
    write(&dest.join("2. video/igual.mov"), b"mismo");
    let inv = scan(&[src.clone()], &AtomicBool::new(false), |_, _| {}).unwrap();
    let plan = build_plan(&inv, &PlanOptions::default(), &Overrides::default());
    let a = analyze(&plan, &dest, true, DefaultConflictAction::Ask, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(a.conflicts.len(), 2);
    let diff = a.conflicts.iter().find(|c| c.needs_decision).unwrap();
    let mut res = HashMap::new();
    res.insert(diff.id, Resolution::KeepBoth);
    let ops = build_operations(&plan, &a, &res, DuplicatePolicy::CopyAll, &dest).unwrap();
    let manifest = JobManifest {
        job_id: "c".into(),
        created_at: String::new(),
        kind: "organize".into(),
        client: String::new(),
        project: String::new(),
        dest_root: dest.clone(),
        sources: vec![src.clone()],
        ops: ops.ops,
        dirs: ops.dirs,
        excluded: ops.excluded,
        settings: CopySettings::default(),
        warnings: vec![],
    };
    let out = run_job(&manifest, &d.path().join("j.jsonl"), &JobControl::default(), None, |_| {}).unwrap();
    let r = build_report(&manifest, &out, "x".into());
    assert!(r.success, "{:#?}", r.entries);
    assert_eq!(std::fs::read(dest.join("2. video/clip.mov")).unwrap(), b"version vieja");
    assert_eq!(std::fs::read(dest.join("2. video/clip_copia1.mov")).unwrap(), b"version nueva");
    assert_eq!(r.totals.skipped, 1);
    assert!(manifest.ops.iter().any(|o| matches!(o.action, OpAction::Skip { .. })));
}

#[test]
fn pause_and_resume_in_same_session() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("src");
    for i in 0..6 {
        write(&src.join(format!("c{i}.mov")), &pseudo_random(300_000, i));
    }
    let dest = d.path().join("P");
    let mut run = prepare(&src, &dest, d.path(), PlanOptions::default(), VerifyMode::Full);
    run.manifest.settings.concurrency = 1;
    let control = Arc::new(JobControl::default());
    control.pause.store(true, Ordering::SeqCst);
    let c2 = control.clone();
    let saw_paused = Arc::new(AtomicBool::new(false));
    let sp = saw_paused.clone();
    let t = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        c2.pause.store(false, Ordering::SeqCst);
    });
    let out = run_job(&run.manifest, &run.journal, &control, None, |p| {
        if p.paused {
            sp.store(true, Ordering::Relaxed);
        }
    })
    .unwrap();
    t.join().unwrap();
    assert!(saw_paused.load(Ordering::Relaxed));
    assert!(build_report(&run.manifest, &out, "x".into()).success);
}

/// Requiere un sistema de archivos pequeño montado en WPO_SMALL_FS_DIR
/// (p. ej. `mount -t tmpfs -o size=2m tmpfs /ruta`). Se omite si no existe.
#[test]
fn out_of_space_aborts_and_is_reported() {
    let Ok(small) = std::env::var("WPO_SMALL_FS_DIR") else {
        eprintln!("WPO_SMALL_FS_DIR no definido: prueba omitida");
        return;
    };
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("src");
    for i in 0..6 {
        write(&src.join(format!("c{i}.mov")), &pseudo_random(700_000, i));
    }
    let dest = PathBuf::from(small).join(format!("P{}", std::process::id()));
    let inv = scan(&[src.clone()], &AtomicBool::new(false), |_, _| {}).unwrap();
    let plan = build_plan(&inv, &PlanOptions::default(), &Overrides::default());
    let a = analyze(&plan, &dest, false, DefaultConflictAction::Ask, &AtomicBool::new(false), |_| {}).unwrap();
    let ops = build_operations(&plan, &a, &HashMap::new(), DuplicatePolicy::CopyAll, &dest).unwrap();
    // La verificación previa ya lo detecta.
    let pf = run_preflight(&ops, &[src.clone()], &dest, &AtomicBool::new(false), |_, _| {});
    assert!(pf.issues.iter().any(|i| i.code == "no-space"));
    // Y si se fuerza la copia, el motor se detiene limpiamente.
    let manifest = JobManifest {
        job_id: "s".into(),
        created_at: String::new(),
        kind: "organize".into(),
        client: String::new(),
        project: String::new(),
        dest_root: dest.clone(),
        sources: vec![src.clone()],
        ops: ops.ops,
        dirs: ops.dirs,
        excluded: ops.excluded,
        settings: CopySettings { concurrency: 1, ..Default::default() },
        warnings: vec![],
    };
    let out = run_job(&manifest, &d.path().join("j.jsonl"), &JobControl::default(), None, |_| {}).unwrap();
    let r = build_report(&manifest, &out, "x".into());
    assert_eq!(r.status, ReportStatus::Aborted);
    assert!(!r.success);
    assert!(r.aborted_reason.unwrap().contains("Sin espacio"));
    assert_eq!(wpo_core::copy::cleanup_temp_files(&dest), 0, "no deben quedar temporales");
    let _ = std::fs::remove_dir_all(&dest);
}

#[cfg(unix)]
#[test]
fn unreadable_source_files_fail_without_affecting_others() {
    use std::os::unix::fs::PermissionsExt;
    if unsafe { libc_geteuid() } == 0 {
        eprintln!("ejecutando como root: prueba de permisos omitida");
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("src");
    write(&src.join("ok.wav"), b"ok");
    write(&src.join("bloqueado.wav"), b"no");
    std::fs::set_permissions(src.join("bloqueado.wav"), std::fs::Permissions::from_mode(0o000)).unwrap();
    let dest = d.path().join("P");
    let inv = scan(&[src.clone()], &AtomicBool::new(false), |_, _| {}).unwrap();
    let plan = build_plan(&inv, &PlanOptions::default(), &Overrides::default());
    let a = analyze(&plan, &dest, true, DefaultConflictAction::Ask, &AtomicBool::new(false), |_| {}).unwrap();
    let ops = build_operations(&plan, &a, &HashMap::new(), DuplicatePolicy::CopyAll, &dest).unwrap();
    let pf = run_preflight(&ops, &[src.clone()], &dest, &AtomicBool::new(false), |_, _| {});
    assert!(pf.issues.iter().any(|i| i.code == "unreadable" && i.count == 1));
    let manifest = JobManifest {
        job_id: "p".into(),
        created_at: String::new(),
        kind: "organize".into(),
        client: String::new(),
        project: String::new(),
        dest_root: dest.clone(),
        sources: vec![src.clone()],
        ops: ops.ops,
        dirs: ops.dirs,
        excluded: ops.excluded,
        settings: CopySettings::default(),
        warnings: vec![],
    };
    let out = run_job(&manifest, &d.path().join("j.jsonl"), &JobControl::default(), None, |_| {}).unwrap();
    let r = build_report(&manifest, &out, "x".into());
    std::fs::set_permissions(src.join("bloqueado.wav"), std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(r.totals.failed, 1);
    assert_eq!(r.totals.copied, 1);
    assert_eq!(r.status, ReportStatus::CompletedWithErrors);
    assert!(!r.success);
}

#[cfg(unix)]
extern "C" {
    #[link_name = "geteuid"]
    fn libc_geteuid() -> u32;
}
