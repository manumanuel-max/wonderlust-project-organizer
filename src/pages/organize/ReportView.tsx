import { save } from "@tauri-apps/plugin-dialog";
import { CheckCircle2, Download, FolderOpen, RotateCcw, XCircle } from "lucide-react";
import { useEffect, useState } from "react";
import { Badge, Button, Card, Modal, PathText, Spinner, Stat, Tabs } from "../../components/ui";
import { api } from "../../lib/api";
import { formatDate, formatDuration, formatNumber, formatSize } from "../../lib/format";
import { useStore } from "../../lib/store";
import type { JobReport, ReportEntry } from "../../lib/types";

type EntryTab = "error" | "pendiente" | "omitido" | "copiado" | "excluido";

function Entries({ jobId, report }: { jobId: string; report: JobReport }) {
  const t = report.totals;
  const initial: EntryTab = t.failed ? "error" : t.pending ? "pendiente" : t.skipped ? "omitido" : "copiado";
  const [tab, setTab] = useState<EntryTab>(initial);
  const [entries, setEntries] = useState<ReportEntry[]>([]);
  const [total, setTotal] = useState(0);
  const [search, setSearch] = useState("");

  useEffect(() => {
    if (tab === "excluido") return;
    const h = setTimeout(() => {
      api
        .reportEntries(jobId, tab, search, 0, 500)
        .then((p) => {
          setEntries(p.entries);
          setTotal(p.total);
        })
        .catch(() => {});
    }, 120);
    return () => clearTimeout(h);
  }, [jobId, tab, search]);

  return (
    <Card className="overflow-hidden">
      <div className="flex items-center gap-3 px-4">
        <div className="flex-1">
          <Tabs
            value={tab}
            onChange={setTab}
            tabs={[
              { value: "error", label: "Errores", count: t.failed, tone: "danger" },
              { value: "pendiente", label: "Pendientes", count: t.pending, tone: "warn" },
              { value: "omitido", label: "Omitidos", count: t.skipped },
              { value: "copiado", label: report.transfer?.mode === "move" ? "Procesados" : "Copiados", count: t.copied },
              { value: "excluido", label: "Excluidos", count: t.excluded },
            ]}
          />
        </div>
        {tab !== "excluido" && (
          <input
            placeholder="Buscar…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="h-7 w-48 rounded-md border border-line bg-bg px-2 text-xs focus:border-accent/60 focus:outline-none"
          />
        )}
      </div>
      <div className="max-h-[360px] overflow-auto">
        {tab === "excluido" ? (
          report.excluded.length === 0 ? (
            <p className="py-8 text-center text-xs text-muted">Sin archivos excluidos.</p>
          ) : (
            report.excluded.map((e) => (
              <div key={e.itemId} className="flex items-center gap-3 border-b border-line/60 px-4 py-2">
                <span className="mono min-w-0 flex-1 truncate text-xs" title={e.relSrc}>
                  {e.relSrc}
                </span>
                <Badge>{e.reason}</Badge>
                <span className="w-20 text-right text-xs text-muted">{formatSize(e.size)}</span>
              </div>
            ))
          )
        ) : entries.length === 0 ? (
          <p className="py-8 text-center text-xs text-muted">Nada que mostrar.</p>
        ) : (
          entries.map((e) => (
            <div key={e.idx} className="border-b border-line/60 px-4 py-2">
              <div className="flex items-center gap-3">
                <span className="mono min-w-0 flex-1 truncate text-xs" title={e.source}>
                  {e.source}
                </span>
                <span className="w-20 shrink-0 text-right text-xs text-muted">{formatSize(e.size)}</span>
              </div>
              <div className="mono truncate text-2xs text-faint" title={e.dest}>
                → {e.destRel} {e.renamed && "(copia renombrada)"}
              </div>
              <div className={e.status === "error" ? "text-2xs text-danger" : "text-2xs text-muted"}>{e.detail}</div>
            </div>
          ))
        )}
        {tab !== "excluido" && total > entries.length && (
          <p className="py-2 text-center text-2xs text-faint">Mostrando {entries.length} de {formatNumber(total)}. Exporta el reporte para la lista completa.</p>
        )}
      </div>
    </Card>
  );
}

export function ReportBody({ jobId, onRetry }: { jobId: string; onRetry?: () => void }) {
  const toast = useStore((s) => s.toast);
  const [report, setReport] = useState<JobReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [retrying, setRetrying] = useState(false);

  useEffect(() => {
    setReport(null);
    api
      .getReport(jobId)
      .then(setReport)
      .catch((e) => setError(e.message));
  }, [jobId]);

  if (error) return <p className="py-10 text-center text-xs text-muted">{error}</p>;
  if (!report) return <div className="flex justify-center py-12"><Spinner /></div>;
  const t = report.totals;
  const moving = report.transfer?.mode === "move";

  const exportAs = async (format: "csv" | "json" | "txt" | "pathmap") => {
    const ext = format === "pathmap" ? "csv" : format;
    const base = format === "pathmap" ? "mapa_de_rutas" : "reporte";
    const name = `${base}_${[report.client, report.project].filter(Boolean).join("_") || report.jobId}.${ext}`.replace(/[\\/:*?"<>|]/g, "_");
    const path = await save({ defaultPath: name, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
    if (!path) return;
    try {
      await api.exportReport(report.jobId, format, path);
      toast("success", "Reporte exportado.");
    } catch (e) {
      toast("error", (e as Error).message);
    }
  };

  const retry = async () => {
    setRetrying(true);
    try {
      const n = await api.retryFailed(report.jobId);
      toast("info", `Reintentando ${n} archivo(s).`);
      useStore.getState().patchOrganize({ jobId: report.jobId, step: "copy", client: report.client, project: report.project });
      onRetry?.();
    } catch (e) {
      toast("error", (e as Error).message);
    } finally {
      setRetrying(false);
    }
  };

  return (
    <div className="space-y-5">
      <Card className={report.success ? "border-ok/30 p-6" : "border-danger/30 p-6"}>
        <div className="flex items-start gap-4">
          <div className={`flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl ${report.success ? "bg-ok/15 text-ok" : "bg-danger/15 text-danger"}`}>
            {report.success ? <CheckCircle2 className="h-6 w-6" /> : <XCircle className="h-6 w-6" />}
          </div>
          <div className="min-w-0 flex-1">
            <div className="text-lg font-semibold tracking-tight">{report.success ? (moving ? "Movimiento completado y verificado" : "Backup completado y verificado") : report.statusLabel}</div>
            <p className="selectable mt-0.5 text-[13px] text-muted">{report.integrityMessage}</p>
            {report.abortedReason && <p className="mt-1 text-[13px] text-danger">Motivo: {report.abortedReason}</p>}
            <div className="mt-2 text-xs text-faint">
              {report.kindLabel} · {[report.client, report.project].filter(Boolean).join(" — ")} · {formatDate(report.finishedAt)}
            </div>
          </div>
        </div>
        <div className="mt-6 grid grid-cols-2 gap-5 border-t border-line pt-5 md:grid-cols-7">
          <Stat label={moving ? "Procesados" : "Copiados"} value={formatNumber(t.copied)} sub={formatSize(t.bytesCopied)} tone="ok" />
          <Stat label="Verificados" value={formatNumber(t.verified)} sub={report.integrityMode === "full" ? "SHA-256" : "por tamaño"} />
          <Stat label="Omitidos" value={formatNumber(t.skipped)} />
          <Stat label="Duplicados" value={formatNumber(t.duplicates)} sub="omitidos" />
          <Stat label="Excluidos" value={formatNumber(t.excluded)} sub={formatSize(t.bytesExcluded)} />
          <Stat label="Errores" value={formatNumber(t.failed + t.pending)} tone={t.failed + t.pending ? "danger" : undefined} sub={t.pending ? `${t.pending} pendientes` : undefined} />
          <Stat label="Tiempo" value={formatDuration(report.durationMs)} />
        </div>
        {moving && (
          <div className="mt-5 grid grid-cols-2 gap-5 border-t border-line pt-5 md:grid-cols-5">
            <Stat label="Movidos (mismo disco)" value={formatNumber(t.movedSameDisk ?? 0)} />
            <Stat label="Originales a Papelera" value={formatNumber(t.originalsTrashed ?? 0)} />
            <Stat label="Originales eliminados" value={formatNumber(t.originalsDeleted ?? 0)} />
            <Stat
              label="Originales no retirados"
              value={formatNumber(t.originalsNotRemoved ?? 0)}
              tone={t.originalsNotRemoved ? "warn" : undefined}
              sub={t.originalsNotRemoved ? "siguen en su sitio" : undefined}
            />
            <Stat label="Carpetas vacías retiradas" value={formatNumber(t.prunedDirs ?? 0)} />
          </div>
        )}
        <div className="mt-5 flex flex-wrap items-center gap-2 border-t border-line pt-5">
          <div className="mr-auto min-w-0 max-w-[50%]">
            <div className="text-2xs uppercase tracking-wider text-faint">Destino</div>
            <PathText path={report.destRoot} />
          </div>
          {(t.failed > 0 || t.pending > 0) && (
            <Button variant="primary" icon={<RotateCcw className="h-4 w-4" />} loading={retrying} onClick={retry}>
              Reintentar fallidos
            </Button>
          )}
          <Button icon={<FolderOpen className="h-4 w-4" />} onClick={() => api.openPath(report.destRoot).catch((e) => toast("error", e.message))}>
            Abrir destino
          </Button>
          <Button icon={<Download className="h-4 w-4" />} onClick={() => exportAs("txt")}>
            TXT
          </Button>
          <Button icon={<Download className="h-4 w-4" />} onClick={() => exportAs("csv")}>
            CSV
          </Button>
          <Button icon={<Download className="h-4 w-4" />} onClick={() => exportAs("json")}>
            JSON
          </Button>
          <Button icon={<Download className="h-4 w-4" />} onClick={() => exportAs("pathmap")}>
            Mapa de rutas
          </Button>
        </div>
      </Card>

      <Entries jobId={report.jobId} report={report} />

      {report.warnings.length > 0 && (
        <Card className="p-5">
          <div className="mb-2 text-[13px] font-semibold">Advertencias registradas</div>
          <ul className="selectable list-disc space-y-1 pl-5 text-xs text-muted">
            {report.warnings.map((w, i) => (
              <li key={i}>{w}</li>
            ))}
          </ul>
        </Card>
      )}
    </div>
  );
}

export default function ReportStep() {
  const jobId = useStore((s) => s.organize.jobId);
  if (!jobId) return null;
  return (
    <div className="h-full overflow-auto px-8 pb-10">
      <div className="mx-auto max-w-[1100px]">
        <ReportBody jobId={jobId} />
      </div>
    </div>
  );
}

/** Reporte abierto desde Inicio o Historial. */
export function ReportModal() {
  const jobId = useStore((s) => s.reportJobId);
  const close = () => useStore.getState().openReport(null);
  return (
    <Modal open={!!jobId} onClose={close} title="Reporte de operación" width="max-w-5xl">
      {jobId && (
        <ReportBody
          jobId={jobId}
          onRetry={() => {
            close();
            useStore.getState().setRoute("organize");
          }}
        />
      )}
    </Modal>
  );
}
