import { Pause, Play, Square } from "lucide-react";
import { useEffect, useState } from "react";
import { Button, Card, Modal, PathText, ProgressBar, Stat } from "../../components/ui";
import { api, events } from "../../lib/api";
import { formatDuration, formatNumber, formatSize, formatSpeed, percent } from "../../lib/format";
import { useStore } from "../../lib/store";
import type { JobStatus, Progress } from "../../lib/types";

export default function CopyStep() {
  const jobId = useStore((s) => s.organize.jobId);
  const patch = useStore((s) => s.patchOrganize);
  const toast = useStore((s) => s.toast);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [status, setStatus] = useState<JobStatus | null>(null);
  const [cancelOpen, setCancelOpen] = useState(false);

  useEffect(() => {
    if (!jobId) return;
    api
      .jobStatus(jobId)
      .then((s) => {
        setStatus(s);
        if (s.progress) setProgress(s.progress);
        if (!s.running) patch({ step: "report" });
      })
      .catch(() => {});
    const un = events.onJobProgress((e) => {
      if (e.jobId === jobId) setProgress(e.progress);
    });
    return () => {
      un.then((f) => f());
    };
  }, [jobId, patch]);

  if (!jobId) return null;
  const p = progress;
  const pct = p ? percent(p.bytesDone, p.bytesTotal) : 0;
  const moving = status?.transfer?.mode === "move";

  const act = (fn: () => Promise<void>) => fn().catch((e) => toast("error", e.message));

  return (
    <div className="h-full overflow-auto px-8 pb-10">
      <div className="mx-auto max-w-[980px] space-y-5">
        <Card className="p-6">
          <div className="flex items-start justify-between gap-6">
            <div className="min-w-0">
              <div className="text-2xs uppercase tracking-wider text-faint">{status?.kind ?? "Copia"}</div>
              <div className="mt-1 truncate text-lg font-semibold tracking-tight">
                {[status?.client, status?.project].filter(Boolean).join(" — ") || (moving ? "Moviendo proyecto" : "Copiando proyecto")}
              </div>
              {status && <PathText path={status.destRoot} className="mt-0.5" />}
            </div>
            <div className="text-right">
              <div className="text-3xl font-semibold tabular-nums tracking-tight">{pct.toFixed(1).replace(".", ",")} %</div>
              <div className="text-xs text-muted">{p?.paused ? "En pausa" : moving ? "Moviendo y verificando" : "Copiando y verificando"}</div>
            </div>
          </div>
          <ProgressBar value={pct} className="mt-5 h-2.5" tone={p?.paused ? "warn" : "accent"} animated={!p?.paused} />
          <div className="mt-6 grid grid-cols-2 gap-5 md:grid-cols-6">
            <Stat label={moving ? "Procesado" : "Transferido"} value={formatSize(p?.bytesDone ?? 0)} sub={`de ${formatSize(p?.bytesTotal ?? 0)}`} />
            <Stat label="Archivos" value={`${formatNumber(p?.filesDone ?? 0)}`} sub={`de ${formatNumber(p?.filesTotal ?? 0)}`} />
            <Stat label="Velocidad" value={formatSpeed(p?.speedBps ?? 0)} />
            <Stat label="Transcurrido" value={formatDuration(p?.elapsedMs ?? 0)} />
            <Stat label="Restante" value={p?.paused ? "—" : formatDuration(p?.etaMs ?? null)} />
            <Stat label="Errores" value={formatNumber(p?.filesFailed ?? 0)} tone={p?.filesFailed ? "danger" : undefined} />
          </div>
          <div className="mt-6 flex justify-end gap-2 border-t border-line pt-5">
            {p?.paused ? (
              <Button variant="primary" icon={<Play className="h-4 w-4" />} onClick={() => act(() => api.resumeJob(jobId))}>
                Reanudar
              </Button>
            ) : (
              <Button icon={<Pause className="h-4 w-4" />} onClick={() => act(() => api.pauseJob(jobId))}>
                Pausar
              </Button>
            )}
            <Button variant="danger" icon={<Square className="h-3.5 w-3.5" />} onClick={() => setCancelOpen(true)}>
              Cancelar
            </Button>
          </div>
        </Card>

        <Card className="p-5">
          <div className="mb-3 text-[13px] font-semibold">Archivos en curso</div>
          {!p || p.active.length === 0 ? (
            <p className="py-4 text-center text-xs text-muted">{p?.paused ? "En pausa." : "Preparando…"}</p>
          ) : (
            <div className="space-y-3">
              {p.active.map((f) => (
                <div key={f.idx}>
                  <div className="mb-1 flex items-center justify-between gap-4 text-xs">
                    <span className="truncate">{f.name}</span>
                    <span className="shrink-0 tabular-nums text-muted">
                      {f.phase} · {formatSize(f.done)} / {formatSize(f.total)}
                    </span>
                  </div>
                  <ProgressBar value={f.phase === "Verificando" ? 100 : percent(f.done, f.total)} tone={f.phase === "Verificando" ? "ok" : "accent"} animated={f.phase === "Verificando"} />
                </div>
              ))}
            </div>
          )}
          <p className="mt-4 text-xs text-muted">
            Puedes seguir usando la aplicación: la copia continúa en segundo plano. Si se cierra la aplicación, podrás reanudar desde Inicio sin repetir archivos
            ya verificados.
          </p>
        </Card>
      </div>

      <Modal
        open={cancelOpen}
        onClose={() => setCancelOpen(false)}
        title={moving ? "¿Cancelar el movimiento?" : "¿Cancelar la copia?"}
        footer={
          <>
            <Button variant="ghost" onClick={() => setCancelOpen(false)}>
              Seguir copiando
            </Button>
            <Button
              variant="danger"
              onClick={() => {
                setCancelOpen(false);
                act(() => api.cancelJob(jobId));
              }}
            >
              {moving ? "Cancelar movimiento" : "Cancelar copia"}
            </Button>
          </>
        }
      >
        <p className="text-[13px] text-muted">
          El archivo en curso se descarta de forma segura. Los archivos ya verificados permanecen en el destino y podrás reanudar más tarde. La operación quedará
          marcada como incompleta.
          {moving && " Los archivos que aún no se procesaron siguen en su ubicación original."}
        </p>
      </Modal>
    </div>
  );
}
