import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Archive, File, FilePlus2, Folder, FolderPlus, Layers, Search, Trash2, UploadCloud } from "lucide-react";
import { useEffect, useState } from "react";
import { DestinationPicker } from "../../components/DestinationPicker";
import { Button, Card, cx, Field, Input, PathText, SectionTitle, Spinner } from "../../components/ui";
import { api, events } from "../../lib/api";
import { basename, formatNumber, formatSize, todayIso } from "../../lib/format";
import { useStore } from "../../lib/store";
import type { OrganizeMode } from "../../lib/types";
import { optionsFromSettings, projectFolderName } from "./common";

function DropZone({ onPaths }: { onPaths: (p: string[]) => void }) {
  const [over, setOver] = useState(false);
  useEffect(() => {
    let un: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === "over" || p.type === "enter") setOver(true);
        else if (p.type === "leave") setOver(false);
        else if (p.type === "drop") {
          setOver(false);
          if (p.paths.length) onPaths(p.paths);
        }
      })
      .then((f) => (un = f))
      .catch(() => {});
    return () => un?.();
  }, [onPaths]);
  return (
    <div
      className={cx(
        "flex flex-col items-center justify-center rounded-2xl border-2 border-dashed px-6 py-10 text-center transition-all duration-150",
        over ? "scale-[1.005] border-accent bg-accent/10" : "border-line bg-bg/40",
      )}
    >
      <UploadCloud className={cx("mb-3 h-8 w-8", over ? "text-accent" : "text-faint")} />
      <p className="text-[13px] font-medium">Arrastra aquí archivos o carpetas completas</p>
      <p className="mt-1 text-xs text-muted">Se analizan de forma recursiva. Los originales solo se leen.</p>
    </div>
  );
}

function ModeCard({ active, onClick, icon, title, text }: { active: boolean; onClick: () => void; icon: React.ReactNode; title: string; text: string }) {
  return (
    <button
      onClick={onClick}
      className={cx(
        "flex flex-1 items-start gap-3 rounded-xl border p-4 text-left transition-colors",
        active ? "border-accent/60 bg-accent/[0.07]" : "border-line hover:border-faint",
      )}
    >
      <span className={cx("mt-0.5", active ? "text-accent" : "text-muted")}>{icon}</span>
      <span>
        <span className="block text-[13px] font-medium">{title}</span>
        <span className="mt-0.5 block text-xs text-muted">{text}</span>
      </span>
    </button>
  );
}

export default function SourcesStep() {
  const o = useStore((s) => s.organize);
  const patch = useStore((s) => s.patchOrganize);
  const settings = useStore((s) => s.settings)!;
  const toast = useStore((s) => s.toast);
  const [scanning, setScanning] = useState(false);
  const [progress, setProgress] = useState({ files: 0, bytes: 0 });
  const [folderName, setFolderName] = useState("");

  useEffect(() => {
    if (!o.destParent && settings.recentDestinations[0]) patch({ destParent: settings.recentDestinations[0] });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const rootName = o.sources.length === 1 ? basename(o.sources[0]) : "";
  useEffect(() => {
    projectFolderName(o.date, o.client, o.project, rootName).then(setFolderName).catch(() => {});
  }, [o.date, o.client, o.project, rootName]);

  const addPaths = async (paths: string[]) => {
    const merged = Array.from(new Set([...useStore.getState().organize.sources, ...paths]));
    patch({ sources: merged });
    // Autocompletar datos si la carpeta ya sigue la nomenclatura.
    if (merged.length === 1 && !o.client && !o.project) {
      const parsed = await api.parseProjectName(basename(merged[0])).catch(() => null);
      if (parsed) patch({ date: parsed.date, client: parsed.client, project: parsed.project });
    }
  };

  const pickFiles = async () => {
    const r = await open({ multiple: true, directory: false, title: "Seleccionar archivos" });
    if (Array.isArray(r)) addPaths(r);
    else if (typeof r === "string") addPaths([r]);
  };
  const pickFolders = async () => {
    const r = await open({ multiple: true, directory: true, title: "Seleccionar carpetas" });
    if (Array.isArray(r)) addPaths(r);
    else if (typeof r === "string") addPaths([r]);
  };

  const analyze = async () => {
    setScanning(true);
    setProgress({ files: 0, bytes: 0 });
    const un = await events.onScanProgress(setProgress);
    try {
      const plan = await api.scanSources(o.sources, optionsFromSettings(settings, o.mode));
      const p: Partial<typeof o> = { plan, analysis: null, step: "review" };
      if (plan.suggestedName && !o.client && !o.project) {
        p.date = plan.suggestedName.date;
        p.client = plan.suggestedName.client;
        p.project = plan.suggestedName.project;
      }
      if (plan.summary.alreadyOrganized && o.mode === "organize") {
        toast("info", "El proyecto ya sigue la estructura estandarizada: se conservará la ubicación de sus archivos.");
      }
      patch(p);
    } catch (e) {
      toast("error", (e as Error).message);
    } finally {
      un();
      setScanning(false);
    }
  };

  const ready = o.sources.length > 0 && !!o.destParent;

  return (
    <div className="h-full overflow-auto px-8 pb-10">
      <div className="mx-auto grid max-w-[1180px] grid-cols-1 gap-5 lg:grid-cols-[1.25fr_1fr]">
        <Card className="p-5">
          <SectionTitle
            hint="Selecciona un proyecto completo o un conjunto de archivos y carpetas."
            right={
              o.sources.length > 0 && (
                <Button size="sm" variant="ghost" onClick={() => patch({ sources: [] })}>
                  Quitar todo
                </Button>
              )
            }
          >
            Contenido a organizar
          </SectionTitle>
          <DropZone onPaths={addPaths} />
          <div className="mt-3 flex gap-2">
            <Button icon={<FilePlus2 className="h-4 w-4" />} onClick={pickFiles}>
              Añadir archivos
            </Button>
            <Button icon={<FolderPlus className="h-4 w-4" />} onClick={pickFolders}>
              Añadir carpetas
            </Button>
          </div>
          {o.sources.length > 0 && (
            <div className="mt-4 max-h-[260px] divide-y divide-line overflow-auto rounded-xl border border-line">
              {o.sources.map((p) => (
                <div key={p} className="group flex items-center gap-3 px-3 py-2">
                  <span className="text-muted">{/\.[^\\/]{1,8}$/.test(p) ? <File className="h-4 w-4" /> : <Folder className="h-4 w-4" />}</span>
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[13px]">{basename(p)}</div>
                    <PathText path={p} className="text-2xs" />
                  </div>
                  <button
                    className="rounded p-1 text-faint opacity-0 hover:bg-raised hover:text-danger group-hover:opacity-100"
                    onClick={() => patch({ sources: o.sources.filter((x) => x !== p) })}
                    aria-label="Quitar"
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </button>
                </div>
              ))}
            </div>
          )}
          <div className="mt-5">
            <div className="mb-2 text-xs font-medium text-muted">Modo</div>
            <div className="flex gap-3">
              <ModeCard
                active={o.mode === "organize"}
                onClick={() => patch({ mode: "organize" as OrganizeMode })}
                icon={<Layers className="h-4 w-4" />}
                title="Organizar en estructura estándar"
                text="Clasifica en las cinco carpetas conservando subcarpetas funcionales."
              />
              <ModeCard
                active={o.mode === "preserveHierarchy"}
                onClick={() => patch({ mode: "preserveHierarchy" as OrganizeMode })}
                icon={<Archive className="h-4 w-4" />}
                title="Backup conservando jerarquía"
                text="Copia la estructura original tal cual. Recomendado si hay proyectos de Adobe."
              />
            </div>
          </div>
        </Card>

        <div className="space-y-5">
          <Card className="space-y-4 p-5">
            <SectionTitle hint="Se usa para nombrar la carpeta del proyecto en el destino.">Información del proyecto</SectionTitle>
            <div className="grid grid-cols-[150px_1fr] gap-3">
              <Field label="Fecha">
                <Input type="date" value={o.date} onChange={(e) => patch({ date: e.target.value || todayIso() })} />
              </Field>
              <Field label="Cliente">
                <Input placeholder="Banco Popular" value={o.client} onChange={(e) => patch({ client: e.target.value })} />
              </Field>
            </div>
            <Field label="Proyecto">
              <Input placeholder="Campaña Navidad" value={o.project} onChange={(e) => patch({ project: e.target.value })} />
            </Field>
          </Card>
          <Card className="space-y-3 p-5">
            <SectionTitle hint="Disco local, SSD/HDD externo, memoria USB, unidad de red o Google Drive para escritorio.">Destino</SectionTitle>
            <DestinationPicker value={o.destParent} onChange={(p) => patch({ destParent: p })} />
            {folderName && o.destParent && (
              <div className="rounded-lg bg-bg px-3 py-2.5">
                <div className="text-2xs uppercase tracking-wider text-faint">Carpeta del proyecto</div>
                <div className="mt-0.5 truncate text-[13px] font-medium">{folderName}</div>
              </div>
            )}
          </Card>
          <div className="flex items-center justify-end gap-3">
            {scanning && (
              <div className="flex items-center gap-2 text-xs text-muted">
                <Spinner />
                {formatNumber(progress.files)} archivos · {formatSize(progress.bytes)}
                <Button size="sm" variant="ghost" onClick={() => api.cancelScan()}>
                  Cancelar
                </Button>
              </div>
            )}
            <Button variant="primary" size="lg" icon={<Search className="h-4 w-4" />} disabled={!ready} loading={scanning} onClick={analyze}>
              Analizar contenido
            </Button>
          </div>
          {!ready && <p className="text-right text-xs text-faint">Añade contenido y elige un destino para continuar.</p>}
        </div>
      </div>
    </div>
  );
}
