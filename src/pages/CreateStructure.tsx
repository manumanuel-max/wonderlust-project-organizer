import { CheckCircle2, Folder, FolderOpen, FolderPlus, RotateCcw } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { DestinationPicker } from "../components/DestinationPicker";
import { PageHeader } from "../components/Shell";
import { Badge, Button, Card, CAT_DOT, cx, Field, Input, PathText } from "../components/ui";
import { api } from "../lib/api";
import { CATEGORIES, joinPath, todayIso } from "../lib/format";
import { useStore } from "../lib/store";
import type { StructureCheck, StructureResult } from "../lib/types";

export default function CreateStructure() {
  const toast = useStore((s) => s.toast);
  const settings = useStore((s) => s.settings);
  const [date, setDate] = useState(todayIso());
  const [client, setClient] = useState("");
  const [project, setProject] = useState("");
  const [parent, setParent] = useState(settings?.recentDestinations[0] ?? "");
  const [name, setName] = useState("");
  const [problems, setProblems] = useState<string[]>([]);
  const [check, setCheck] = useState<StructureCheck | null>(null);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<StructureResult | null>(null);

  useEffect(() => {
    let alive = true;
    api
      .previewProjectName(date, client, project)
      .then((p) => {
        if (!alive) return;
        setName(p.name);
        setProblems(p.problems);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [date, client, project]);

  useEffect(() => {
    if (!parent || !name) return setCheck(null);
    api.checkStructure(parent, name).then(setCheck).catch(() => setCheck(null));
  }, [parent, name, result]);

  const fullPath = useMemo(() => (parent && name ? joinPath(parent, name) : ""), [parent, name]);
  const canCreate = !!parent && client.trim() && project.trim() && name && check?.parentExists && check.nameProblems.length === 0;

  const create = async (completeExisting: boolean) => {
    setBusy(true);
    try {
      const r = await api.createStructure({ parent, name, client, project, completeExisting });
      setResult(r);
      toast("success", r.created.length === 5 ? "Estructura creada." : `Se completaron ${r.created.length} carpeta(s) faltante(s).`);
    } catch (e) {
      toast("error", (e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const reset = () => {
    setResult(null);
    setClient("");
    setProject("");
    setDate(todayIso());
  };

  return (
    <div className="h-full overflow-auto">
      <PageHeader title="Crear estructura" subtitle="Genera la carpeta del proyecto con las cinco subcarpetas estandarizadas." />
      <div className="mx-auto grid max-w-[1180px] grid-cols-1 gap-5 px-8 pb-10 lg:grid-cols-[1fr_420px]">
        <Card className="p-6">
          {result ? (
            <div className="flex flex-col items-center py-10 text-center">
              <div className="mb-4 flex h-14 w-14 items-center justify-center rounded-2xl bg-ok/15 text-ok">
                <CheckCircle2 className="h-7 w-7" />
              </div>
              <h2 className="text-lg font-semibold">Estructura lista</h2>
              <PathText path={result.root} className="mt-2 max-w-full" />
              {result.existing.length > 0 && (
                <p className="mt-2 text-xs text-muted">Ya existían y no se modificaron: {result.existing.join(", ")}</p>
              )}
              <div className="mt-6 flex gap-2">
                <Button variant="primary" icon={<FolderOpen className="h-4 w-4" />} onClick={() => api.openPath(result.root).catch((e) => toast("error", e.message))}>
                  Abrir en {navigator.userAgent.includes("Mac") ? "Finder" : "el Explorador"}
                </Button>
                <Button icon={<RotateCcw className="h-4 w-4" />} onClick={reset}>
                  Crear otra
                </Button>
              </div>
            </div>
          ) : (
            <div className="space-y-5">
              <div className="grid grid-cols-[180px_1fr] gap-4">
                <Field label="Fecha del proyecto">
                  <Input type="date" value={date} onChange={(e) => setDate(e.target.value || todayIso())} />
                </Field>
                <Field label="Cliente">
                  <Input autoFocus placeholder="Banco Popular" value={client} onChange={(e) => setClient(e.target.value)} />
                </Field>
              </div>
              <Field label="Nombre del proyecto">
                <Input placeholder="Campaña Navidad" value={project} onChange={(e) => setProject(e.target.value)} onKeyDown={(e) => e.key === "Enter" && canCreate && !check?.rootExists && create(false)} />
              </Field>
              <Field label="Ubicación de destino">
                <DestinationPicker value={parent} onChange={setParent} />
              </Field>

              {check && !check.parentExists && <p className="text-xs text-danger">La ubicación no existe o la unidad no está conectada.</p>}
              {check?.rootExists && (
                <div className="rounded-lg border border-warn/30 bg-warn/10 px-3 py-2.5 text-xs text-warn">
                  Ya existe una carpeta con este nombre.{" "}
                  {check.missingFolders.length > 0
                    ? `Faltan ${check.missingFolders.length} subcarpeta(s); puedes completarlas sin modificar nada de lo existente.`
                    : "Ya tiene las cinco subcarpetas."}
                </div>
              )}
              {check?.nameProblems.map((p) => (
                <p key={p} className="text-xs text-danger">
                  El nombre {p}
                </p>
              ))}

              <div className="flex items-center justify-end gap-2 border-t border-line pt-5">
                {check?.rootExists ? (
                  <Button variant="primary" disabled={!canCreate || check.missingFolders.length === 0} loading={busy} onClick={() => create(true)}>
                    Completar carpetas faltantes
                  </Button>
                ) : (
                  <Button variant="primary" size="lg" icon={<FolderPlus className="h-4 w-4" />} disabled={!canCreate} loading={busy} onClick={() => create(false)}>
                    Crear estructura
                  </Button>
                )}
              </div>
            </div>
          )}
        </Card>

        <Card className="p-6">
          <div className="text-2xs uppercase tracking-wider text-faint">Vista previa</div>
          <div className="mt-3 break-words text-[15px] font-semibold leading-snug tracking-tight">{name || "DD.MM.AA CLIENTE - PROYECTO"}</div>
          {problems.length > 0 && !result && (
            <div className="mt-2 flex flex-wrap gap-1">
              {problems.map((p) => (
                <Badge key={p} tone="warn">
                  {p}
                </Badge>
              ))}
            </div>
          )}
          <div className="mono mt-5 rounded-xl border border-line bg-bg p-4 leading-7">
            <div className="flex items-center gap-2 text-fg">
              <Folder className="h-4 w-4 text-accent" />
              <span className="truncate">{name || "…"}/</span>
            </div>
            {CATEGORIES.map((c, i) => (
              <div key={c.key} className="flex items-center gap-2 pl-2 text-muted">
                <span className="text-faint">{i === CATEGORIES.length - 1 ? "└──" : "├──"}</span>
                <span className={cx("h-2 w-2 rounded-full", CAT_DOT[c.key])} />
                <span className={check?.rootExists && !check.missingFolders.includes(c.folder) ? "text-faint line-through" : ""}>{c.folder}/</span>
              </div>
            ))}
          </div>
          {fullPath && (
            <div className="mt-4">
              <div className="mb-1 text-2xs uppercase tracking-wider text-faint">Ruta completa</div>
              <PathText path={fullPath} />
            </div>
          )}
          <p className="mt-5 text-xs leading-relaxed text-muted">
            Nomenclatura: <span className="mono">{settings?.naming.template}</span>. Puedes cambiarla en Configuración.
          </p>
        </Card>
      </div>
    </div>
  );
}
