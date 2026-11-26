import { AlertTriangle, ArrowRight, Cloud, FolderPlus, FolderTree, HardDrive, History, Home as HomeIcon, Play, Star } from "lucide-react";
import { useEffect, useState } from "react";
import { PageHeader } from "../components/Shell";
import { Badge, Button, Card, cx, ProgressBar } from "../components/ui";
import { api } from "../lib/api";
import { formatDate, formatSize, KIND_LABELS, percent, STATUS_LABELS } from "../lib/format";
import { useStore } from "../lib/store";
import type { HistoryEntry, InterruptedJob, Location } from "../lib/types";

export function statusTone(status: string): "ok" | "warn" | "danger" | "neutral" | "accent" {
  if (status === "completed" || status === "created") return "ok";
  if (status === "running") return "accent";
  if (status === "cancelled" || status === "interrupted") return "warn";
  if (status === "aborted" || status === "completedWithErrors") return "danger";
  return "neutral";
}

function ActionCard({ icon, title, description, onClick, accent }: { icon: React.ReactNode; title: string; description: string; onClick: () => void; accent?: boolean }) {
  return (
    <button
      onClick={onClick}
      className={cx(
        "group relative flex min-h-[190px] flex-col overflow-hidden rounded-2xl border p-6 text-left transition-all duration-200",
        accent ? "border-accent/30 bg-gradient-to-br from-accent/[0.12] to-transparent hover:border-accent/60" : "border-line bg-panel hover:border-faint",
      )}
    >
      <div className={cx("mb-auto flex h-11 w-11 items-center justify-center rounded-xl", accent ? "bg-accent text-onaccent" : "bg-raised text-fg")}>{icon}</div>
      <div className="mt-8">
        <div className="flex items-center gap-2 text-lg font-semibold tracking-tight">
          {title}
          <ArrowRight className="h-4 w-4 -translate-x-1 text-muted opacity-0 transition-all group-hover:translate-x-0 group-hover:opacity-100" />
        </div>
        <p className="mt-1.5 max-w-sm text-[13px] text-muted">{description}</p>
      </div>
    </button>
  );
}

export default function Home() {
  const setRoute = useStore((s) => s.setRoute);
  const openReport = useStore((s) => s.openReport);
  const toast = useStore((s) => s.toast);
  const organize = useStore((s) => s.organize);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [interrupted, setInterrupted] = useState<InterruptedJob[]>([]);
  const [locations, setLocations] = useState<Location[]>([]);

  const refresh = () => {
    api.historyList().then((h) => setHistory(h.slice(0, 6))).catch(() => {});
    api.listInterruptedJobs().then(setInterrupted).catch(() => {});
    api.listLocations().then(setLocations).catch(() => {});
  };
  useEffect(refresh, []);

  const resume = async (j: InterruptedJob) => {
    try {
      await api.resumeInterruptedJob(j.jobId);
      useStore.getState().patchOrganize({ jobId: j.jobId, step: "copy", client: j.client, project: j.project });
      setRoute("organize");
    } catch (e) {
      toast("error", (e as Error).message);
    }
  };
  const discard = async (j: InterruptedJob) => {
    await api.discardInterruptedJob(j.jobId).catch(() => {});
    toast("info", "Se descartó la reanudación. Los archivos ya copiados permanecen en el destino.");
    refresh();
  };

  const places = locations.filter((l) => l.available && l.kind !== "recent").slice(0, 8);

  return (
    <div className="h-full overflow-auto">
      <PageHeader title="Inicio" subtitle="Estandariza, organiza y respalda proyectos audiovisuales." />
      <div className="mx-auto max-w-[1180px] space-y-6 px-8 pb-10">
        {organize.jobId && organize.step === "copy" && (
          <Card className="flex items-center gap-4 border-accent/40 p-4">
            <span className="h-2 w-2 animate-pulse rounded-full bg-accent" />
            <div className="flex-1 text-[13px]">Hay una copia en curso.</div>
            <Button variant="primary" size="sm" onClick={() => setRoute("organize")}>
              Ver progreso
            </Button>
          </Card>
        )}

        <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
          <ActionCard
            accent
            icon={<FolderPlus className="h-5 w-5" />}
            title="Crear estructura"
            description="Genera las carpetas estandarizadas para comenzar un nuevo proyecto."
            onClick={() => setRoute("create")}
          />
          <ActionCard
            icon={<FolderTree className="h-5 w-5" />}
            title="Organizar proyecto"
            description="Clasifica, copia y prepara un proyecto existente para su backup."
            onClick={() => setRoute("organize")}
          />
        </div>

        {interrupted.length > 0 && (
          <Card className="border-warn/30 p-5">
            <div className="mb-3 flex items-center gap-2 text-[13px] font-semibold">
              <AlertTriangle className="h-4 w-4 text-warn" /> Operaciones interrumpidas
            </div>
            <div className="space-y-2">
              {interrupted.map((j) => (
                <div key={j.jobId} className="flex items-center gap-4 rounded-lg bg-raised px-4 py-3">
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[13px] font-medium">
                      {[j.client, j.project].filter(Boolean).join(" — ") || j.destRoot}
                    </div>
                    <div className="truncate text-xs text-muted">
                      {j.reason} · {formatDate(j.createdAt)} · {j.filesDone} de {j.filesTotal} archivos verificados
                    </div>
                    <ProgressBar value={percent(j.filesDone, j.filesTotal)} tone="warn" className="mt-2" />
                  </div>
                  <Button size="sm" variant="ghost" onClick={() => discard(j)}>
                    Descartar
                  </Button>
                  <Button size="sm" variant="primary" icon={<Play className="h-3.5 w-3.5" />} onClick={() => resume(j)}>
                    Reanudar
                  </Button>
                </div>
              ))}
            </div>
            <p className="mt-3 text-xs text-muted">Al reanudar no se repiten los archivos ya copiados y verificados.</p>
          </Card>
        )}

        <div className="grid grid-cols-1 gap-4 lg:grid-cols-[1.5fr_1fr]">
          <Card className="p-5">
            <div className="mb-3 flex items-center justify-between">
              <div className="flex items-center gap-2 text-[13px] font-semibold">
                <History className="h-4 w-4 text-muted" /> Operaciones recientes
              </div>
              <Button size="sm" variant="ghost" onClick={() => setRoute("history")}>
                Ver historial
              </Button>
            </div>
            {history.length === 0 ? (
              <p className="py-6 text-center text-xs text-muted">Aún no hay operaciones.</p>
            ) : (
              <div className="divide-y divide-line">
                {history.map((h) => (
                  <div key={h.id} className="flex items-center gap-3 py-2.5">
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-[13px]">{[h.client, h.project].filter(Boolean).join(" — ") || h.destination}</div>
                      <div className="truncate text-xs text-muted">
                        {KIND_LABELS[h.kind] ?? h.kind} · {formatDate(h.date)}
                        {h.files > 0 && ` · ${h.files.toLocaleString("es-ES")} archivos · ${formatSize(h.size)}`}
                      </div>
                    </div>
                    <Badge tone={statusTone(h.status)}>{STATUS_LABELS[h.status] ?? h.status}</Badge>
                    {h.jobId && h.status !== "running" && h.status !== "interrupted" ? (
                      <Button size="sm" variant="ghost" onClick={() => openReport(h.jobId)}>
                        Reporte
                      </Button>
                    ) : (
                      <Button size="sm" variant="ghost" onClick={() => api.openPath(h.destination).catch((e) => toast("error", e.message))}>
                        Abrir
                      </Button>
                    )}
                  </div>
                ))}
              </div>
            )}
          </Card>

          <Card className="p-5">
            <div className="mb-3 flex items-center gap-2 text-[13px] font-semibold">
              <Star className="h-4 w-4 text-muted" /> Ubicaciones
            </div>
            {places.length === 0 ? (
              <p className="py-6 text-center text-xs text-muted">Conecta un disco externo o añade rutas frecuentes en Configuración.</p>
            ) : (
              <div className="space-y-1">
                {places.map((l) => (
                  <button
                    key={l.kind + l.path}
                    onClick={() => api.openPath(l.path).catch((e) => toast("error", e.message))}
                    className="flex w-full items-center gap-3 rounded-lg px-2 py-2 text-left hover:bg-raised"
                    title={l.path}
                  >
                    <span className="text-muted">
                      {l.kind === "volume" ? <HardDrive className="h-4 w-4" /> : l.kind === "gdrive" ? <Cloud className="h-4 w-4" /> : l.kind === "frequent" ? <Star className="h-4 w-4" /> : <HomeIcon className="h-4 w-4" />}
                    </span>
                    <span className="min-w-0 flex-1 truncate text-[13px]">{l.label}</span>
                    {l.freeBytes != null && l.totalBytes ? (
                      <span className="w-24">
                        <ProgressBar value={100 - percent(l.freeBytes, l.totalBytes)} tone={l.freeBytes / l.totalBytes < 0.1 ? "danger" : "accent"} />
                        <span className="mt-1 block text-right text-2xs text-faint">{formatSize(l.freeBytes)} libres</span>
                      </span>
                    ) : null}
                  </button>
                ))}
              </div>
            )}
          </Card>
        </div>
      </div>
    </div>
  );
}
