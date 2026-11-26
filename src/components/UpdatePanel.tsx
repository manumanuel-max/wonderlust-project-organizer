import { CheckCircle2, Download, RefreshCw, RotateCw } from "lucide-react";
import { useEffect, useState } from "react";
import { api, events } from "../lib/api";
import { formatSize, percent } from "../lib/format";
import { useStore } from "../lib/store";
import { Badge, Button, ProgressBar } from "./ui";

type Info = { version: string; currentVersion: string; date: string | null; notes: string | null };
type Phase = "idle" | "checking" | "uptodate" | "available" | "installing" | "ready" | "error";

/** Buscar actualizaciones: comprobar, descargar, verificar la firma, instalar y reiniciar. */
export function UpdatePanel() {
  const [cfg, setCfg] = useState<{ configured: boolean; currentVersion: string } | null>(null);
  const [phase, setPhase] = useState<Phase>("idle");
  const [info, setInfo] = useState<Info | null>(null);
  const [error, setError] = useState("");
  const [prog, setProg] = useState<{ downloaded: number; total: number | null }>({ downloaded: 0, total: null });
  const requested = useStore((s) => s.checkUpdatesRequested);

  useEffect(() => {
    api.updateConfig().then(setCfg).catch(() => {});
  }, []);

  const check = async () => {
    setPhase("checking");
    setError("");
    try {
      const r = await api.checkUpdate();
      setInfo(r);
      setPhase(r ? "available" : "uptodate");
    } catch (e) {
      setError((e as Error).message);
      setPhase("error");
    }
  };

  useEffect(() => {
    if (requested && cfg?.configured) {
      useStore.setState({ checkUpdatesRequested: false });
      check();
    }
  }, [requested, cfg]);

  const install = async () => {
    setPhase("installing");
    setProg({ downloaded: 0, total: null });
    const un = await events.onUpdateProgress(setProg);
    try {
      await api.installUpdate();
      setPhase("ready");
    } catch (e) {
      setError((e as Error).message);
      setPhase("error");
    } finally {
      un();
    }
  };

  if (!cfg) return null;
  return (
    <div className="space-y-4">
      <div className="flex items-center gap-3 text-[13px]">
        Versión instalada <Badge>{cfg.currentVersion}</Badge>
      </div>
      {!cfg.configured ? (
        <p className="text-xs text-muted">
          Esta compilación no tiene un canal de actualizaciones configurado (las versiones generadas por GitHub Actions sí lo incluyen).
        </p>
      ) : (
        <>
          <div className="flex items-center gap-2">
            <Button icon={<RefreshCw className="h-4 w-4" />} loading={phase === "checking"} disabled={phase === "installing"} onClick={check}>
              Buscar actualizaciones
            </Button>
            {phase === "uptodate" && (
              <span className="flex items-center gap-1.5 text-xs text-ok">
                <CheckCircle2 className="h-4 w-4" /> Ya tienes la versión más reciente.
              </span>
            )}
          </div>
          {(phase === "available" || phase === "installing" || phase === "ready") && info && (
            <div className="rounded-xl border border-accent/30 bg-accent/[0.05] p-4">
              <div className="text-[13px] font-medium">
                Nueva versión disponible: {info.version} <span className="text-muted">(tienes {info.currentVersion})</span>
              </div>
              {info.notes && <p className="selectable mt-2 whitespace-pre-line text-xs text-muted">{info.notes}</p>}
              {phase === "installing" && (
                <div className="mt-3">
                  <ProgressBar value={prog.total ? percent(prog.downloaded, prog.total) : 5} animated={!prog.total} />
                  <div className="mt-1 text-2xs text-muted">
                    Descargando y verificando la firma… {formatSize(prog.downloaded)}
                    {prog.total ? ` de ${formatSize(prog.total)}` : ""}
                  </div>
                </div>
              )}
              <div className="mt-3 flex gap-2">
                {phase === "available" && (
                  <Button variant="primary" icon={<Download className="h-4 w-4" />} onClick={install}>
                    Descargar e instalar
                  </Button>
                )}
                {phase === "ready" && (
                  <Button variant="primary" icon={<RotateCw className="h-4 w-4" />} onClick={() => api.restartApp().catch((e) => setError(e.message))}>
                    Reiniciar para terminar
                  </Button>
                )}
              </div>
            </div>
          )}
          {phase === "error" && <p className="selectable text-xs text-danger">{error}</p>}
          <p className="text-xs text-muted">
            Cada actualización está firmada digitalmente por Wonderlust; si la firma no coincide, se rechaza y no se instala nada. No se puede actualizar
            mientras hay una copia o un movimiento en curso.
          </p>
        </>
      )}
    </div>
  );
}
