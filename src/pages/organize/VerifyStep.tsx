import { AlertOctagon, AlertTriangle, ArrowLeft, Copy, Info, MoveRight, RefreshCw, ShieldCheck } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Badge, Button, Card, Checkbox, cx, Input, Modal, PathText, ProgressBar, Select, Spinner, Stat } from "../../components/ui";
import { api, events } from "../../lib/api";
import { formatNumber, formatSize, formatUnix, percent } from "../../lib/format";
import { useStore } from "../../lib/store";
import type { Conflict, PreflightIssue, RemovalMode, Resolution, TransferMode } from "../../lib/types";
import { destinationRoot } from "./common";
import { WarningCard } from "./ReviewStep";

const KIND_LABEL: Record<Conflict["kind"], string> = {
  existingIdentical: "Ya existe idéntico en el destino",
  existingDifferent: "Mismo nombre en el destino, contenido diferente",
  internalIdentical: "Duplicado idéntico con el mismo destino",
  internalDifferent: "Mismo nombre, contenido diferente",
};

function IssueRow({ issue }: { issue: PreflightIssue }) {
  const [open, setOpen] = useState(false);
  const icon =
    issue.level === "blocking" ? <AlertOctagon className="h-4 w-4 text-danger" /> : issue.level === "warning" ? <AlertTriangle className="h-4 w-4 text-warn" /> : <Info className="h-4 w-4 text-muted" />;
  return (
    <div className={cx("rounded-xl border px-4 py-3", issue.level === "blocking" ? "border-danger/30 bg-danger/[0.06]" : issue.level === "warning" ? "border-warn/30 bg-warn/[0.05]" : "border-line")}>
      <div className="flex items-start gap-3">
        <span className="mt-0.5">{icon}</span>
        <div className="min-w-0 flex-1">
          <div className="text-[13px] font-medium">{issue.title}</div>
          <p className="selectable mt-0.5 text-xs text-muted">{issue.detail}</p>
          {issue.paths.length > 0 && (
            <button className="mt-1 text-xs text-accent hover:underline" onClick={() => setOpen(!open)}>
              {open ? "Ocultar" : `Ver ${issue.count > issue.paths.length ? `los primeros ${issue.paths.length} de ${issue.count}` : issue.count} elemento(s)`}
            </button>
          )}
          {open && (
            <div className="mono selectable mt-2 max-h-48 overflow-auto rounded-lg bg-bg p-2 text-2xs leading-5 text-muted">
              {issue.paths.map((p) => (
                <div key={p} className="truncate" title={p}>
                  {p}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function resolutionValue(r: Resolution | null): string {
  if (!r) return "";
  return r.type;
}

export default function VerifyStep() {
  const o = useStore((s) => s.organize);
  const patch = useStore((s) => s.patchOrganize);
  const toast = useStore((s) => s.toast);
  const settings = useStore((s) => s.settings)!;
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<{ phase: string; done: number; total: number } | null>(null);
  const [destRoot, setDestRoot] = useState("");
  const [applyAll, setApplyAll] = useState(true);
  const [renameFor, setRenameFor] = useState<Conflict | null>(null);
  const [newName, setNewName] = useState("");
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [ack, setAck] = useState(false);
  const [mode, setMode] = useState<TransferMode>("copy");
  const [removal, setRemoval] = useState<RemovalMode>(settings.moveRemoval ?? "trash");
  const [moveAck, setMoveAck] = useState(false);
  const [starting, setStarting] = useState(false);
  const started = useRef(false);
  const a = o.analysis;
  const plan = o.plan!;

  const run = useCallback(async () => {
    const root = await destinationRoot(o.destParent, o.date, o.client, o.project, plan.rootName);
    setDestRoot(root);
    setRunning(true);
    setProgress(null);
    const un = await events.onAnalysisProgress(setProgress);
    try {
      const res = await api.runAnalysis(root);
      patch({ analysis: res });
    } catch (e) {
      toast("error", (e as Error).message);
    } finally {
      un();
      setRunning(false);
    }
  }, [o.destParent, o.date, o.client, o.project, plan.rootName, patch, toast]);

  useEffect(() => {
    if (started.current) return;
    started.current = true;
    if (!a) run();
    else setDestRoot(a.destRoot);
  }, [a, run]);

  const resolve = async (c: Conflict, r: Resolution | null) => {
    try {
      const res = await api.resolveConflicts([{ id: c.id, resolution: r }], applyAll && r?.type !== "rename");
      if (a) patch({ analysis: { ...a, conflicts: res.conflicts, unresolved: res.unresolved } });
    } catch (e) {
      toast("error", (e as Error).message);
    }
  };

  const start = async () => {
    setStarting(true);
    try {
      const id = await api.startJob(o.client, o.project, { mode, removal });
      setConfirmOpen(false);
      patch({ jobId: id, step: "copy" });
    } catch (e) {
      toast("error", (e as Error).message);
    } finally {
      setStarting(false);
    }
  };

  const pf = a?.preflight;
  const blocking = pf?.issues.filter((i) => i.level === "blocking") ?? [];
  const decisive = a?.conflicts.filter((c) => c.needsDecision) ?? [];
  const automatic = a?.conflicts.filter((c) => !c.needsDecision) ?? [];
  const adobeWarning = plan.warnings.find((w) => w.code === "adobe-relink");
  const canStart = !!a && !running && blocking.length === 0 && a.unresolved === 0 && (pf?.filesToCopy ?? 0) > 0;

  return (
    <div className="h-full overflow-auto px-8 pb-10">
      <div className="mx-auto max-w-[1180px] space-y-5">
        <Card className="p-5">
          <div className="flex items-start gap-4">
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-raised text-accent">
              <ShieldCheck className="h-5 w-5" />
            </div>
            <div className="min-w-0 flex-1">
              <div className="text-[13px] font-semibold">Verificación previa</div>
              <p className="text-xs text-muted">Permisos, espacio, rutas, caracteres, duplicados por contenido (SHA-256) y conflictos con el destino.</p>
              <div className="mt-2">
                <div className="text-2xs uppercase tracking-wider text-faint">Destino final</div>
                <PathText path={destRoot || "…"} className="!text-fg" />
              </div>
            </div>
            <Button icon={<RefreshCw className="h-4 w-4" />} onClick={run} disabled={running}>
              Volver a verificar
            </Button>
          </div>
          {running && (
            <div className="mt-4">
              <div className="mb-1.5 flex items-center justify-between text-xs text-muted">
                <span className="flex items-center gap-2">
                  <Spinner className="h-3.5 w-3.5" /> {progress?.phase ?? "Preparando…"}
                </span>
                <span className="flex items-center gap-3">
                  {progress && progress.total > 0 && `${formatNumber(progress.done)} / ${formatNumber(progress.total)}`}
                  <Button size="sm" variant="ghost" onClick={() => api.cancelAnalysis()}>
                    Cancelar
                  </Button>
                </span>
              </div>
              <ProgressBar value={progress ? percent(progress.done, progress.total) : 3} animated />
            </div>
          )}
          {pf && !running && (
            <div className="mt-5 grid grid-cols-2 gap-4 border-t border-line pt-4 md:grid-cols-5">
              <Stat label="A copiar" value={formatNumber(pf.filesToCopy)} sub={formatSize(pf.requiredBytes)} tone="accent" />
              <Stat label="Omitidos" value={formatNumber(pf.filesSkipped)} sub="por duplicado/conflicto" />
              <Stat
                label="Espacio libre"
                value={formatSize(pf.availableBytes)}
                sub={pf.availableBytes != null ? `${percent(pf.requiredBytes, pf.availableBytes).toFixed(0)} % se usará` : "desconocido"}
                tone={pf.availableBytes != null && pf.requiredBytes > pf.availableBytes ? "danger" : undefined}
              />
              <Stat label="Sistema de archivos" value={pf.filesystem?.toUpperCase() ?? "—"} />
              <Stat label="Duplicados" value={formatNumber(a?.duplicates.length ?? 0)} sub="grupos idénticos" />
            </div>
          )}
        </Card>

        {pf && !running && pf.issues.length > 0 && (
          <div className="space-y-2">
            {pf.issues.map((i) => (
              <IssueRow key={i.code} issue={i} />
            ))}
          </div>
        )}

        {adobeWarning && <WarningCard w={adobeWarning} />}

        {a && !running && decisive.length > 0 && (
          <Card className="overflow-hidden">
            <div className="flex items-center justify-between border-b border-line px-5 py-3">
              <div>
                <div className="text-[13px] font-semibold">Conflictos de nombre · {decisive.length}</div>
                <p className="text-xs text-muted">Nunca se sobrescribe un archivo existente. Elige qué hacer con cada copia nueva.</p>
              </div>
              <label className="flex items-center gap-2 text-xs text-muted">
                <Checkbox checked={applyAll} onChange={setApplyAll} /> Aplicar a conflictos equivalentes
              </label>
            </div>
            <div className="max-h-[420px] divide-y divide-line overflow-auto">
              {decisive.map((c) => (
                <div key={c.id} className="grid grid-cols-[1fr_1fr_200px] gap-4 px-5 py-3">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="truncate text-[13px] font-medium">{c.name}</span>
                      <Badge tone="warn">{KIND_LABEL[c.kind]}</Badge>
                    </div>
                    <div className="mt-1 text-2xs text-faint">Origen</div>
                    <PathText path={c.source.path} className="text-2xs" />
                    <div className="text-2xs text-muted">
                      {formatSize(c.source.size)} · {formatUnix(c.source.mtime)}
                    </div>
                  </div>
                  <div className="min-w-0">
                    <div className="text-2xs text-faint">{c.kind.startsWith("existing") ? "Existente en destino" : "Choca con"}</div>
                    <PathText path={c.other.path} className="text-2xs" />
                    <div className="text-2xs text-muted">
                      {formatSize(c.other.size)} · {formatUnix(c.other.mtime)}
                    </div>
                    <div className="mt-1 text-2xs text-muted">Comparación: {c.comparison}</div>
                    <div className="mono truncate text-2xs text-faint">→ {c.destRel}</div>
                  </div>
                  <div>
                    <Select
                      value={resolutionValue(c.resolution)}
                      onChange={(v) => {
                        if (v === "rename") {
                          setRenameFor(c);
                          setNewName(c.name.replace(/(\.[^.]+)?$/, "_nuevo$1"));
                        } else resolve(c, v ? ({ type: v } as Resolution) : null);
                      }}
                      className={cx("w-full", !c.resolution && "border-warn/60")}
                      options={[
                        { value: "", label: "Elegir…" },
                        { value: "skip", label: "Omitir" },
                        { value: "keepBoth", label: "Conservar ambos" },
                        { value: "rename", label: "Renombrar la copia…" },
                      ]}
                    />
                    {c.resolution?.type === "rename" && <div className="mono mt-1 truncate text-2xs text-muted">→ {c.resolution.name}</div>}
                    {c.resolution?.type === "keepBoth" && <div className="mt-1 text-2xs text-muted">La copia nueva recibe el sufijo «_copiaN».</div>}
                  </div>
                </div>
              ))}
            </div>
          </Card>
        )}

        {a && !running && (automatic.length > 0 || a.duplicates.length > 0) && (
          <Card className="p-5">
            <div className="text-[13px] font-semibold">Duplicados por contenido</div>
            <p className="mt-0.5 text-xs text-muted">
              {automatic.length > 0 && `${automatic.length} archivo(s) idénticos a uno existente o con el mismo destino se omitirán. `}
              {a.duplicates.length > 0 &&
                (settings.duplicates.policy === "skipIdentical"
                  ? "Se copiará solo la primera aparición de cada contenido idéntico (Configuración → Duplicados)."
                  : "Los archivos idénticos con nombres o ubicaciones distintas se copian todos para no romper vínculos (Configuración → Duplicados).")}
            </p>
            {a.duplicates.length > 0 && (
              <div className="mt-3 max-h-64 space-y-2 overflow-auto">
                {a.duplicates.slice(0, 200).map((d) => (
                  <div key={d.hash} className="rounded-lg bg-bg px-3 py-2">
                    <div className="flex items-center gap-2 text-xs">
                      <Copy className="h-3.5 w-3.5 text-muted" />
                      <span>{d.rels.length} copias idénticas</span>
                      <span className="text-faint">· {formatSize(d.size)} c/u</span>
                      <span className="mono ml-auto text-2xs text-faint">SHA-256 {d.hash.slice(0, 12)}…</span>
                    </div>
                    {d.rels.map((r) => (
                      <div key={r} className="mono truncate pl-5 text-2xs text-muted" title={r}>
                        {r}
                      </div>
                    ))}
                  </div>
                ))}
              </div>
            )}
          </Card>
        )}

        <div className="flex items-center justify-between pt-2">
          <Button variant="ghost" icon={<ArrowLeft className="h-4 w-4" />} onClick={() => patch({ step: "review" })}>
            Volver a la vista previa
          </Button>
          <div className="flex items-center gap-3">
            {a && a.unresolved > 0 && <span className="text-xs text-warn">{a.unresolved} conflicto(s) sin decisión</span>}
            {blocking.length > 0 && <span className="text-xs text-danger">Corrige los problemas bloqueantes</span>}
            <Button
              variant="primary"
              size="lg"
              disabled={!canStart}
              onClick={() => {
                setMode(pf && !pf.copyPossible ? "move" : "copy");
                setAck(false);
                setMoveAck(false);
                setConfirmOpen(true);
              }}
            >
              Continuar
            </Button>
          </div>
        </div>
      </div>

      <Modal
        open={!!renameFor}
        onClose={() => setRenameFor(null)}
        title="Renombrar la copia nueva"
        footer={
          <>
            <Button variant="ghost" onClick={() => setRenameFor(null)}>
              Cancelar
            </Button>
            <Button
              variant="primary"
              disabled={!newName.trim()}
              onClick={() => {
                if (renameFor) resolve(renameFor, { type: "rename", name: newName.trim() });
                setRenameFor(null);
              }}
            >
              Aplicar
            </Button>
          </>
        }
      >
        <p className="mb-3 text-xs text-muted">Solo cambia el nombre de la copia en el destino. El archivo original no se modifica.</p>
        <Input autoFocus value={newName} onChange={(e) => setNewName(e.target.value)} />
      </Modal>

      <Modal
        open={confirmOpen}
        onClose={() => setConfirmOpen(false)}
        title="¿Quieres copiar o mover los archivos?"
        width="max-w-2xl"
        footer={
          <>
            <Button variant="ghost" onClick={() => setConfirmOpen(false)}>
              Cancelar
            </Button>
            <Button
              variant={mode === "move" ? "danger" : "primary"}
              loading={starting}
              disabled={(!!adobeWarning && !ack) || (mode === "move" && !moveAck) || (mode === "copy" && pf?.copyPossible === false)}
              onClick={start}
            >
              {mode === "move" ? "Mover archivos" : "Copiar archivos"}
            </Button>
          </>
        }
      >
        <div className="space-y-4 text-[13px]">
          <div className="grid grid-cols-2 gap-3">
            <button
              onClick={() => pf?.copyPossible !== false && setMode("copy")}
              disabled={pf?.copyPossible === false}
              className={cx(
                "rounded-xl border p-4 text-left transition-colors disabled:cursor-not-allowed disabled:opacity-45",
                mode === "copy" ? "border-accent/60 bg-accent/[0.07]" : "border-line hover:border-faint",
              )}
            >
              <div className="flex items-center gap-2 font-medium">
                <Copy className="h-4 w-4 text-accent" /> Copiar <Badge tone="ok">Recomendado</Badge>
              </div>
              <p className="mt-1.5 text-xs text-muted">
                Los originales se quedan exactamente donde están. Necesita <b className="text-fg">{formatSize(pf?.requiredBytes)}</b> libres en el destino.
              </p>
              {pf?.copyPossible === false && <p className="mt-1.5 text-xs text-danger">No hay espacio suficiente para copiar.</p>}
            </button>
            <button
              onClick={() => setMode("move")}
              className={cx("rounded-xl border p-4 text-left transition-colors", mode === "move" ? "border-warn/60 bg-warn/[0.07]" : "border-line hover:border-faint")}
            >
              <div className="flex items-center gap-2 font-medium">
                <MoveRight className="h-4 w-4 text-warn" /> Mover
              </div>
              <p className="mt-1.5 text-xs text-muted">
                Los archivos salen de su ubicación original. Necesita{" "}
                <b className="text-fg">{pf && pf.requiredBytesMove === 0 ? "0 B (mismo disco)" : formatSize(pf?.requiredBytesMove)}</b> libres.
              </p>
            </button>
          </div>

          <p>
            <b>{formatNumber(pf?.filesToCopy ?? 0)}</b> archivos (<b>{formatSize(pf?.requiredBytes)}</b>) hacia:
          </p>
          <PathText path={destRoot} className="!text-fg" />

          {mode === "copy" ? (
            <ul className="list-disc space-y-1 pl-5 text-xs text-muted">
              <li>Los originales no se moverán, renombrarán, modificarán ni eliminarán.</li>
              <li>Cada archivo se escribe como temporal, se verifica {settings.copy.verify === "full" ? "con SHA-256" : "por tamaño"} y luego se renombra sin sobrescribir.</li>
              <li>Puedes pausar o cancelar; la operación se puede reanudar sin repetir archivos verificados.</li>
            </ul>
          ) : (
            <div className="space-y-3">
              <ul className="list-disc space-y-1 pl-5 text-xs text-muted">
                <li>
                  <b className="text-fg">Mismo disco:</b> el archivo se mueve al instante, sin reescribirlo ni ocupar espacio extra.
                </li>
                <li>
                  <b className="text-fg">Otro disco:</b> se copia, se verifica con SHA-256 y <b className="text-fg">solo entonces</b> se retira el original. Si algo
                  falla, el original se queda donde estaba.
                </li>
                <li>Los archivos excluidos u omitidos (cachés, autosaves, conflictos) no se tocan y permanecen en el origen.</li>
                <li>Las carpetas que queden vacías en el origen se eliminan; la carpeta que seleccionaste se conserva.</li>
              </ul>
              <div>
                <div className="mb-1.5 text-xs font-medium text-muted">Si el destino está en otro disco, los originales verificados…</div>
                <div className="flex gap-2">
                  {(
                    [
                      ["trash", "Van a la Papelera (recuperable)"],
                      ["delete", "Se eliminan definitivamente"],
                    ] as const
                  ).map(([v, label]) => (
                    <button
                      key={v}
                      onClick={() => setRemoval(v)}
                      className={cx(
                        "h-8 rounded-lg border px-3 text-xs",
                        removal === v ? (v === "delete" ? "border-danger/60 bg-danger/10" : "border-accent/60 bg-accent/10") : "border-line text-muted hover:text-fg",
                      )}
                    >
                      {label}
                    </button>
                  ))}
                </div>
              </div>
              {plan.summary.adobeProjects > 0 && (
                <div className="flex gap-2 rounded-lg border border-danger/30 bg-danger/[0.06] p-3 text-xs">
                  <AlertOctagon className="mt-0.5 h-4 w-4 shrink-0 text-danger" />
                  <span>
                    Hay <b>{plan.summary.adobeProjects}</b> proyecto(s) de After Effects / Premiere Pro. Al mover, también el proyecto <b>original</b> perderá la
                    ubicación de sus medios y habrá que revincularlos. Si necesitas abrirlo sin revincular, elige <b>Copiar</b>.
                  </span>
                </div>
              )}
              <label className="flex items-start gap-2 rounded-lg border border-warn/30 bg-warn/[0.06] p-3 text-xs">
                <Checkbox checked={moveAck} onChange={setMoveAck} />
                <span>Entiendo que los archivos dejarán de estar en su ubicación original.</span>
              </label>
            </div>
          )}
          {adobeWarning && (
            <label className="flex items-start gap-2 rounded-lg border border-warn/30 bg-warn/[0.06] p-3 text-xs">
              <Checkbox checked={ack} onChange={setAck} />
              <span>
                Entiendo que esto genera un <b>backup organizado de archivos</b>, no un proyecto recopilado listo para abrir: los proyectos de After Effects /
                Premiere Pro pueden requerir revincular medios. Se guardará un mapa de rutas originales y nuevas.
              </span>
            </label>
          )}
        </div>
      </Modal>
    </div>
  );
}
