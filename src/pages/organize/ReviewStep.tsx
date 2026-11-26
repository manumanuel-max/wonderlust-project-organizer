import { AlertOctagon, AlertTriangle, ArrowLeft, ArrowRight, Info, Layers } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Badge, Button, Card, CAT_DOT, cx, Input, PathText, Stat, Tabs, Toggle } from "../../components/ui";
import { api } from "../../lib/api";
import { CATEGORIES, formatNumber, formatSize } from "../../lib/format";
import { useStore } from "../../lib/store";
import type { ExclusionSettings, FolderGroup, PlanOptions, PlanResponse, PlanSequence, PlanWarning } from "../../lib/types";
import { ItemTable } from "./ItemTable";
import { TreeView } from "./TreeView";
import { projectFolderName } from "./common";

type Tab = "structure" | "files" | "sequences" | "folders" | "exclusions" | "warnings";

export function WarningCard({ w, compact }: { w: PlanWarning; compact?: boolean }) {
  const [open, setOpen] = useState(!compact);
  const tone = w.level === "critical" ? "border-danger/30 bg-danger/[0.06]" : w.level === "warning" ? "border-warn/30 bg-warn/[0.06]" : "border-line bg-raised/50";
  const icon =
    w.level === "critical" ? <AlertOctagon className="h-4 w-4 text-danger" /> : w.level === "warning" ? <AlertTriangle className="h-4 w-4 text-warn" /> : <Info className="h-4 w-4 text-muted" />;
  return (
    <div className={cx("flex gap-3 rounded-xl border px-4 py-3", tone)}>
      <span className="mt-0.5 shrink-0">{icon}</span>
      <div className="min-w-0">
        <div className="flex items-center gap-3">
          <div className="text-[13px] font-medium">{w.title}</div>
          {compact && (
            <button className="text-xs text-accent hover:underline" onClick={() => setOpen(!open)}>
              {open ? "Ocultar" : "Ver detalles"}
            </button>
          )}
        </div>
        {open ? (
          <p className="selectable mt-1 whitespace-pre-line text-xs leading-relaxed text-muted">{w.message}</p>
        ) : (
          <p className="mt-0.5 truncate text-xs text-muted">{w.message}</p>
        )}
      </div>
    </div>
  );
}

function Sequences({ plan, onPlan }: { plan: PlanResponse; onPlan: (p: PlanResponse) => void }) {
  const toast = useStore((s) => s.toast);
  const decide = (keys: string[], accept: boolean | null) =>
    api
      .setSequenceDecision(keys, accept)
      .then(onPlan)
      .catch((e) => toast("error", e.message));
  const pending = plan.sequences.filter((s) => s.confidence === "ambiguous" && s.decision === null);
  if (plan.sequences.length === 0) return <p className="py-10 text-center text-xs text-muted">No se detectaron secuencias de imágenes.</p>;
  const row = (s: PlanSequence) => (
    <div key={s.key} className="flex items-start gap-4 border-b border-line/60 px-1 py-3">
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="mono text-[13px] text-fg">
            {s.prefix}
            {s.padding ? "#".repeat(s.padding) : "#"}.{s.ext}
          </span>
          {s.confidence === "ambiguous" ? (
            s.decision === null ? <Badge tone="warn">Requiere confirmación</Badge> : <Badge>Decidido</Badge>
          ) : (
            <Badge tone="ok">Detectada</Badge>
          )}
          {s.missingCount > 0 && <Badge tone="danger">{formatNumber(s.missingCount)} fotogramas faltantes</Badge>}
        </div>
        <div className="mt-1 text-xs text-muted">
          {formatNumber(s.count)} fotogramas ({s.first}–{s.last}) · {formatSize(s.totalSize)} · origen: <span className="mono">{s.relDir || "(raíz)"}</span>
          {s.accepted && (
            <>
              {" "}
              → <span className="mono">1. imagen/{s.destDir}</span>
            </>
          )}
        </div>
        {s.ambiguity && <div className="mt-1 text-xs text-warn">{s.ambiguity}</div>}
        {s.missingRanges.length > 0 && (
          <div className="mono mt-1 truncate text-2xs text-faint" title={s.missingRanges.map(([a, b]) => (a === b ? `${a}` : `${a}-${b}`)).join(", ")}>
            Faltan: {s.missingRanges.slice(0, 12).map(([a, b]) => (a === b ? `${a}` : `${a}–${b}`)).join(", ")}
            {s.missingRanges.length > 12 && " …"}
          </div>
        )}
      </div>
      <div className="flex shrink-0 gap-1.5">
        <Button size="sm" variant={s.accepted ? "primary" : "secondary"} onClick={() => decide([s.key], true)}>
          Es secuencia
        </Button>
        <Button size="sm" variant={!s.accepted && (s.decision !== null || s.confidence === "high") ? "primary" : "secondary"} onClick={() => decide([s.key], false)}>
          Imágenes independientes
        </Button>
      </div>
    </div>
  );
  return (
    <div>
      {pending.length > 0 && (
        <div className="mb-3 flex items-center gap-3 rounded-xl border border-warn/30 bg-warn/[0.06] px-4 py-3">
          <AlertTriangle className="h-4 w-4 text-warn" />
          <span className="flex-1 text-[13px]">{pending.length} grupo(s) ambiguos. Indica si son secuencias o imágenes independientes.</span>
          <Button size="sm" onClick={() => decide(pending.map((p) => p.key), false)}>
            Todas independientes
          </Button>
          <Button size="sm" onClick={() => decide(pending.map((p) => p.key), true)}>
            Todas secuencias
          </Button>
        </div>
      )}
      {plan.sequences.map(row)}
    </div>
  );
}

const GROUP_LABELS: Record<string, string> = {
  cameraCard: "Tarjeta de cámara",
  organizedProject: "Proyecto ya organizado",
  proxies: "Proxies",
  renders: "Renders / entregables",
  music: "Música",
  sfx: "Efectos de sonido",
  voiceover: "Locuciones",
  stock: "Material de stock",
  graphics: "Recursos gráficos",
  autosave: "Autosaves",
  cache: "Cachés",
};

function Folders({ plan, onPlan }: { plan: PlanResponse; onPlan: (p: PlanResponse) => void }) {
  const toast = useStore((s) => s.toast);
  if (plan.groups.length === 0) return <p className="py-10 text-center text-xs text-muted">No se detectaron carpetas con una función específica.</p>;
  return (
    <div>
      <p className="mb-3 text-xs text-muted">
        Carpetas cuya relación funcional se conserva en el destino. Desactiva una si prefieres que sus archivos se clasifiquen de forma individual.
      </p>
      {plan.groups.map((g: FolderGroup) => (
        <div key={g.relDir + g.kind} className="flex items-center gap-4 border-b border-line/60 py-3">
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <span className="truncate text-[13px] font-medium">{g.name || "(raíz)"}</span>
              <Badge tone={g.kind === "cameraCard" || g.kind === "organizedProject" ? "accent" : "neutral"}>{GROUP_LABELS[g.kind]}</Badge>
            </div>
            <div className="mt-0.5 text-xs text-muted">
              {g.detail} · {formatNumber(g.fileCount)} archivos · {formatSize(g.size)}
            </div>
            <PathText path={g.relDir || "(raíz del origen)"} className="text-2xs" />
          </div>
          <Toggle
            checked={g.enabled}
            onChange={(v) =>
              api
                .setGroupEnabled(g.relDir, v)
                .then(onPlan)
                .catch((e) => toast("error", e.message))
            }
          />
        </div>
      ))}
    </div>
  );
}

const EXCL: { key: keyof ExclusionSettings; label: string; hint?: string }[] = [
  { key: "aeAutosave", label: "Autosaves de After Effects" },
  { key: "prAutosave", label: "Autosaves de Premiere Pro" },
  { key: "mediaCache", label: "Adobe Media Cache / Media Cache Files", hint: ".cfa, .pek, .mcdb, .ims" },
  { key: "aeCache", label: "Cachés de After Effects", hint: "Disk Cache y carpetas de caché" },
  { key: "renderPreviews", label: "Previsualizaciones de render", hint: "Video/Audio Previews de Premiere" },
  { key: "proxies", label: "Proxies" },
  { key: "tempFiles", label: "Archivos temporales", hint: ".tmp, ~$archivo, descargas parciales" },
  { key: "hiddenFiles", label: "Archivos ocultos" },
  { key: "systemFiles", label: "Archivos del sistema", hint: ".DS_Store, Thumbs.db, ._*" },
  { key: "emptyFolders", label: "Carpetas vacías" },
];

export function ExclusionsPanel({ exclusions, onChange, stats }: { exclusions: ExclusionSettings; onChange: (e: ExclusionSettings) => void; stats?: PlanResponse["summary"] }) {
  const [ext, setExt] = useState(exclusions.customExtensions.join(", "));
  const [dirs, setDirs] = useState(exclusions.customFolders.join(", "));
  useEffect(() => {
    setExt(exclusions.customExtensions.join(", "));
    setDirs(exclusions.customFolders.join(", "));
  }, [exclusions]);
  const split = (s: string) =>
    s
      .split(",")
      .map((x) => x.trim())
      .filter(Boolean);
  return (
    <div className="space-y-4">
      <p className="text-xs text-muted">Las exclusiones solo afectan a la copia. Nada se elimina del origen. Activado = excluir.</p>
      <div className="grid grid-cols-1 gap-x-8 gap-y-3 md:grid-cols-2">
        {EXCL.map((e) => (
          <Toggle key={e.key} checked={exclusions[e.key] as boolean} onChange={(v) => onChange({ ...exclusions, [e.key]: v })} label={e.label} hint={e.hint} />
        ))}
      </div>
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
        <label className="block">
          <span className="mb-1 block text-xs text-muted">Extensiones excluidas (separadas por comas)</span>
          <Input value={ext} onChange={(e) => setExt(e.target.value)} onBlur={() => onChange({ ...exclusions, customExtensions: split(ext) })} placeholder="xmp, lrcat" />
        </label>
        <label className="block">
          <span className="mb-1 block text-xs text-muted">Carpetas excluidas por nombre</span>
          <Input value={dirs} onChange={(e) => setDirs(e.target.value)} onBlur={() => onChange({ ...exclusions, customFolders: split(dirs) })} placeholder="Viejo, No usar" />
        </label>
      </div>
      {stats && stats.excluded.length > 0 && (
        <div className="rounded-xl border border-line">
          <div className="flex justify-between border-b border-line px-4 py-2 text-2xs uppercase tracking-wider text-faint">
            <span>Excluidos en este proyecto</span>
            <span>
              {formatNumber(stats.excludedFiles)} · {formatSize(stats.excludedSize)}
            </span>
          </div>
          {stats.excluded.map((x) => (
            <div key={x.label} className="flex justify-between px-4 py-1.5 text-[13px]">
              <span>{x.label}</span>
              <span className="tabular-nums text-muted">
                {formatNumber(x.count)} · {formatSize(x.size)}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export default function ReviewStep() {
  const o = useStore((s) => s.organize);
  const patch = useStore((s) => s.patchOrganize);
  const toast = useStore((s) => s.toast);
  const plan = o.plan!;
  const [tab, setTab] = useState<Tab>("structure");
  const [version, setVersion] = useState(0);
  const [dir, setDir] = useState("");
  const [rootName, setRootName] = useState("");
  const s = plan.summary;

  useEffect(() => {
    projectFolderName(o.date, o.client, o.project, plan.rootName).then(setRootName);
  }, [o.date, o.client, o.project, plan.rootName]);

  const onPlan = (p: PlanResponse) => {
    patch({ plan: p, analysis: null, mode: p.options.mode });
    setVersion((v) => v + 1);
  };
  const updateOptions = (opts: Partial<PlanOptions>) =>
    api
      .updatePlan({ ...plan.options, ...opts })
      .then(onPlan)
      .catch((e) => toast("error", e.message));

  const base = useMemo(() => ({ destDir: dir || null }), [dir]);
  const baseAll = useMemo(() => ({}), []);
  const critical = plan.warnings.filter((w) => w.level === "critical");
  const blockedBySequences = s.sequencesPending > 0;

  return (
    <div className="flex h-full flex-col px-8 pb-6">
      <div className="mb-4 grid grid-cols-2 gap-4 rounded-xl border border-line bg-panel px-5 py-4 md:grid-cols-4 xl:grid-cols-8">
        <Stat label="Archivos" value={formatNumber(s.totalFiles)} sub={formatSize(s.totalSize)} />
        <Stat label="A copiar" value={formatNumber(s.includedFiles)} sub={formatSize(s.includedSize)} tone="accent" />
        <Stat label="Excluidos" value={formatNumber(s.excludedFiles)} sub={formatSize(s.excludedSize)} />
        <Stat label="Secuencias" value={formatNumber(s.sequences)} sub={s.sequencesPending ? `${s.sequencesPending} por confirmar` : "detectadas"} tone={s.sequencesPending ? "warn" : undefined} />
        <Stat label="Renders" value={formatNumber(s.renders)} sub="Renders finales" />
        <Stat label="Adobe" value={formatNumber(s.adobeProjects)} sub="AEP / PRPROJ" tone={s.adobeProjects && plan.options.mode === "organize" ? "warn" : undefined} />
        <Stat label="Sin clasificar" value={formatNumber(s.unknownExt)} sub="a 5. archivos" />
        <Stat label="Carpetas" value={formatNumber(s.groups)} sub="funcionales" />
      </div>

      <div className="mb-4 flex flex-wrap items-center gap-2">
        {CATEGORIES.map((c) => (
          <div key={c.key} className="flex items-center gap-2 rounded-lg border border-line bg-panel px-3 py-1.5 text-xs">
            <span className={cx("h-2 w-2 rounded-full", CAT_DOT[c.key])} />
            <span className="text-muted">{c.folder}</span>
            <span className="tabular-nums">{formatNumber(s.byCategory[c.key].count)}</span>
            <span className="tabular-nums text-faint">{formatSize(s.byCategory[c.key].size)}</span>
          </div>
        ))}
        <div className="ml-auto flex items-center gap-4">
          <Toggle
            checked={plan.options.mode === "preserveHierarchy"}
            onChange={(v) => updateOptions({ mode: v ? "preserveHierarchy" : "organize" })}
            label="Conservar jerarquía original"
          />
          {plan.options.mode === "organize" && (
            <Toggle checked={plan.options.preserveSubfolders} onChange={(v) => updateOptions({ preserveSubfolders: v })} label="Mantener subcarpetas" />
          )}
        </div>
      </div>

      {critical.map((w) => (
        <div key={w.code} className="mb-3">
          <WarningCard w={w} compact />
        </div>
      ))}

      <Card className="flex min-h-0 flex-1 flex-col overflow-hidden">
        <div className="px-4">
          <Tabs
            value={tab}
            onChange={setTab}
            tabs={[
              { value: "structure", label: "Estructura de destino" },
              { value: "files", label: "Archivos", count: s.totalFiles },
              { value: "sequences", label: "Secuencias", count: plan.sequences.length, tone: s.sequencesPending ? "warn" : undefined },
              { value: "folders", label: "Carpetas detectadas", count: plan.groups.length },
              { value: "exclusions", label: "Exclusiones", count: s.excludedFiles },
              { value: "warnings", label: "Advertencias", count: plan.warnings.length, tone: critical.length ? "danger" : undefined },
            ]}
          />
        </div>
        <div className="min-h-0 flex-1 overflow-hidden">
          {tab === "structure" && (
            <div className="grid h-full grid-cols-[minmax(280px,38%)_1fr]">
              <div className="overflow-auto border-r border-line p-3">
                <TreeView rootName={rootName} version={version} selected={dir} onSelect={setDir} />
              </div>
              <div className="flex min-h-0 flex-col p-3">
                <div className="mb-2 flex items-center gap-2 text-xs text-muted">
                  <Layers className="h-3.5 w-3.5" />
                  <span className="mono truncate">{dir || "Todo el proyecto"}</span>
                </div>
                <div className="min-h-0 flex-1">
                  <ItemTable base={base} version={version} onPlan={onPlan} />
                </div>
              </div>
            </div>
          )}
          {tab === "files" && (
            <div className="h-full p-3">
              <ItemTable base={baseAll} version={version} onPlan={onPlan} />
            </div>
          )}
          {tab === "sequences" && (
            <div className="h-full overflow-auto p-4">
              <Sequences plan={plan} onPlan={onPlan} />
            </div>
          )}
          {tab === "folders" && (
            <div className="h-full overflow-auto p-4">
              <Folders plan={plan} onPlan={onPlan} />
            </div>
          )}
          {tab === "exclusions" && (
            <div className="h-full overflow-auto p-5">
              <ExclusionsPanel exclusions={plan.options.exclusions} onChange={(e) => updateOptions({ exclusions: e })} stats={s} />
            </div>
          )}
          {tab === "warnings" && (
            <div className="h-full space-y-3 overflow-auto p-4">
              {plan.warnings.length === 0 ? <p className="py-10 text-center text-xs text-muted">Sin advertencias.</p> : plan.warnings.map((w) => <WarningCard key={w.code} w={w} />)}
            </div>
          )}
        </div>
      </Card>

      <div className="mt-4 flex items-center justify-between">
        <Button variant="ghost" icon={<ArrowLeft className="h-4 w-4" />} onClick={() => patch({ step: "sources" })}>
          Volver
        </Button>
        <div className="flex items-center gap-3">
          {blockedBySequences && <span className="text-xs text-warn">Confirma las secuencias ambiguas para continuar.</span>}
          <Button variant="primary" size="lg" disabled={blockedBySequences || s.includedFiles === 0} onClick={() => patch({ step: "verify" })}>
            Verificar destino <ArrowRight className="h-4 w-4" />
          </Button>
        </div>
      </div>
    </div>
  );
}
