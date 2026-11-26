import { open } from "@tauri-apps/plugin-dialog";
import { Cloud, Plus, RotateCcw, Save, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { PageHeader } from "../components/Shell";
import { Badge, Button, Card, cx, Field, Input, PathText, Select, Toggle } from "../components/ui";
import { api } from "../lib/api";
import { CATEGORIES } from "../lib/format";
import { applyTheme, useStore } from "../lib/store";
import type { AppSettings, Category, Location } from "../lib/types";
import { ExclusionsPanel } from "./organize/ReviewStep";

const SECTIONS = [
  ["apariencia", "Tema visual"],
  ["nomenclatura", "Nomenclatura"],
  ["reglas", "Reglas de clasificación"],
  ["exclusiones", "Exclusiones predeterminadas"],
  ["secuencias", "Detección de secuencias"],
  ["renders", "Renders finales"],
  ["duplicados", "Duplicados"],
  ["integridad", "Verificación de integridad"],
  ["rendimiento", "Rendimiento"],
  ["rutas", "Rutas frecuentes"],
  ["drive", "Google Drive"],
] as const;

function Section({ id, title, hint, children }: { id: string; title: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <Card id={id} className="scroll-mt-4 p-6">
      <h2 className="text-[14px] font-semibold tracking-tight">{title}</h2>
      {hint && <p className="mt-1 text-xs text-muted">{hint}</p>}
      <div className="mt-5">{children}</div>
    </Card>
  );
}

const csv = (a: string[]) => a.join(", ");
const uncsv = (s: string) =>
  s
    .split(",")
    .map((x) => x.trim())
    .filter(Boolean);

export default function SettingsPage() {
  const stored = useStore((s) => s.settings)!;
  const setSettings = useStore((s) => s.setSettings);
  const toast = useStore((s) => s.toast);
  const [d, setD] = useState<AppSettings>(stored);
  const [saving, setSaving] = useState(false);
  const [newRule, setNewRule] = useState<{ extension: string; category: Category }>({ extension: "", category: "archivos" });
  const [newPath, setNewPath] = useState({ label: "", path: "" });
  const [drives, setDrives] = useState<Location[]>([]);
  const [renderText, setRenderText] = useState(csv(stored.renderPatterns));
  const [cameraText, setCameraText] = useState(csv(stored.sequences.cameraPrefixes));

  useEffect(() => setD(stored), [stored]);
  useEffect(() => {
    api.listLocations().then((l) => setDrives(l.filter((x) => x.kind === "gdrive"))).catch(() => {});
  }, []);

  const dirty = useMemo(() => JSON.stringify(d) !== JSON.stringify(stored), [d, stored]);
  const up = <K extends keyof AppSettings>(k: K, v: AppSettings[K]) => setD({ ...d, [k]: v });

  const saveAll = async () => {
    setSaving(true);
    try {
      const s = await api.saveSettings({ ...d, renderPatterns: uncsv(renderText), sequences: { ...d.sequences, cameraPrefixes: uncsv(cameraText) } });
      setSettings(s);
      applyTheme(s.theme);
      toast("success", "Configuración guardada.");
    } catch (e) {
      toast("error", (e as Error).message);
    } finally {
      setSaving(false);
    }
  };
  const reset = async () => {
    const s = await api.resetSettings();
    setSettings(s);
    setRenderText(csv(s.renderPatterns));
    setCameraText(csv(s.sequences.cameraPrefixes));
    applyTheme(s.theme);
    toast("info", "Se restauraron los valores predeterminados.");
  };

  const ruleError = (() => {
    const e = newRule.extension.trim().replace(/^\./, "").toLowerCase();
    if (!e) return null;
    if (["aep", "aepx", "prproj"].includes(e)) return "Esta extensión siempre se clasifica en «4. editable».";
    return null;
  })();

  return (
    <div className="flex h-full flex-col">
      <PageHeader
        title="Configuración"
        subtitle="Las preferencias se guardan localmente en este equipo."
        right={
          <div className="flex gap-2">
            <Button variant="ghost" icon={<RotateCcw className="h-4 w-4" />} onClick={reset}>
              Restaurar predeterminados
            </Button>
            <Button variant="primary" icon={<Save className="h-4 w-4" />} disabled={!dirty && renderText === csv(stored.renderPatterns) && cameraText === csv(stored.sequences.cameraPrefixes)} loading={saving} onClick={saveAll}>
              Guardar cambios
            </Button>
          </div>
        }
      />
      <div className="mx-auto flex min-h-0 w-full max-w-[1180px] flex-1 gap-6 px-8 pb-8">
        <nav className="hidden w-52 shrink-0 space-y-0.5 overflow-auto lg:block">
          {SECTIONS.map(([id, label]) => (
            <a key={id} href={`#${id}`} className="block rounded-md px-2.5 py-1.5 text-[13px] text-muted hover:bg-raised hover:text-fg">
              {label}
            </a>
          ))}
        </nav>
        <div className="min-w-0 flex-1 space-y-5 overflow-auto pb-10">
          <Section id="apariencia" title="Tema visual">
            <div className="flex gap-2">
              {(["dark", "light", "system"] as const).map((t) => (
                <button
                  key={t}
                  onClick={() => {
                    up("theme", t);
                    applyTheme(t);
                  }}
                  className={cx("h-9 rounded-lg border px-4 text-[13px]", d.theme === t ? "border-accent/60 bg-accent/10" : "border-line text-muted hover:text-fg")}
                >
                  {t === "dark" ? "Oscuro" : t === "light" ? "Claro" : "Según el sistema"}
                </button>
              ))}
            </div>
          </Section>

          <Section id="nomenclatura" title="Nomenclatura de proyectos" hint="Tokens: {DD} {MM} {AA} {AAAA} {CLIENTE} {PROYECTO}.">
            <div className="grid grid-cols-1 gap-4 md:grid-cols-[1fr_auto]">
              <Field label="Plantilla">
                <Input className="mono" value={d.naming.template} onChange={(e) => up("naming", { ...d.naming, template: e.target.value })} />
              </Field>
              <div className="flex items-end pb-2">
                <Toggle checked={d.naming.uppercase} onChange={(v) => up("naming", { ...d.naming, uppercase: v })} label="Mayúsculas" />
              </div>
            </div>
          </Section>

          <Section
            id="reglas"
            title="Reglas de clasificación"
            hint="Reglas adicionales por extensión. «4. editable» es exclusiva de AEP, AEPX y PRPROJ y no admite reglas."
          >
            <div className="mb-4 flex flex-wrap gap-2">
              {d.customRules.length === 0 && <span className="text-xs text-faint">Sin reglas adicionales.</span>}
              {d.customRules.map((r, i) => (
                <span key={i} className="flex items-center gap-2 rounded-lg border border-line bg-bg py-1 pl-2.5 pr-1 text-xs">
                  <span className="mono">.{r.extension}</span>→<span className="text-muted">{CATEGORIES.find((c) => c.key === r.category)?.folder}</span>
                  <button className="rounded p-1 text-faint hover:text-danger" onClick={() => up("customRules", d.customRules.filter((_, j) => j !== i))}>
                    <Trash2 className="h-3 w-3" />
                  </button>
                </span>
              ))}
            </div>
            <div className="flex items-start gap-2">
              <Input className="mono max-w-[160px]" placeholder="xmp" value={newRule.extension} onChange={(e) => setNewRule({ ...newRule, extension: e.target.value })} />
              <Select
                value={newRule.category}
                onChange={(c) => setNewRule({ ...newRule, category: c })}
                options={CATEGORIES.filter((c) => c.key !== "editable").map((c) => ({ value: c.key, label: c.folder }))}
              />
              <Button
                icon={<Plus className="h-4 w-4" />}
                disabled={!newRule.extension.trim() || !!ruleError}
                onClick={() => {
                  const ext = newRule.extension.trim().replace(/^\./, "").toLowerCase();
                  up("customRules", [...d.customRules.filter((r) => r.extension !== ext), { extension: ext, category: newRule.category }]);
                  setNewRule({ extension: "", category: newRule.category });
                }}
              >
                Añadir regla
              </Button>
            </div>
            {ruleError && <p className="mt-2 text-xs text-warn">{ruleError}</p>}
            <div className="mt-5">
              <Toggle checked={d.preserveSubfolders} onChange={(v) => up("preserveSubfolders", v)} label="Conservar subcarpetas originales dentro de cada categoría" hint="Recomendado. Desactívalo para colocar los archivos sueltos directamente en la categoría." />
            </div>
            <div className="mt-4">
              <Field label="Modo predeterminado al organizar">
                <Select
                  value={d.defaultMode}
                  onChange={(v) => up("defaultMode", v)}
                  options={[
                    { value: "organize", label: "Organizar en estructura estándar" },
                    { value: "preserveHierarchy", label: "Backup conservando jerarquía" },
                  ]}
                />
              </Field>
            </div>
          </Section>

          <Section id="exclusiones" title="Exclusiones predeterminadas" hint="Se aplican al iniciar cada análisis; puedes ajustarlas por proyecto en la vista previa.">
            <ExclusionsPanel exclusions={d.exclusions} onChange={(e) => up("exclusions", e)} />
          </Section>

          <Section id="secuencias" title="Detección de secuencias" hint="Los fotogramas nunca se renombran ni se renumeran.">
            <div className="space-y-4">
              <Toggle checked={d.sequences.enabled} onChange={(v) => up("sequences", { ...d.sequences, enabled: v })} label="Detectar secuencias de imágenes" />
              <div className="grid grid-cols-2 gap-4">
                <Field label="Mínimo de fotogramas para aceptar sin preguntar">
                  <Input type="number" min={2} value={d.sequences.minFrames} onChange={(e) => up("sequences", { ...d.sequences, minFrames: Number(e.target.value) || 2 })} />
                </Field>
                <Field label="Densidad mínima (proporción de fotogramas presentes)">
                  <Input type="number" min={0.1} max={1} step={0.05} value={d.sequences.minDensity} onChange={(e) => up("sequences", { ...d.sequences, minDensity: Number(e.target.value) || 0.9 })} />
                </Field>
              </div>
              <Field label="Prefijos de fotos de cámara (se consideran casos ambiguos)">
                <Input className="mono" value={cameraText} onChange={(e) => setCameraText(e.target.value)} />
              </Field>
            </div>
          </Section>

          <Section id="renders" title="Patrones de renders finales" hint="Palabras completas en nombres de carpetas o archivos de video. La detección es una sugerencia que puedes cambiar en la vista previa.">
            <Field label="Patrones (separados por comas)">
              <Input className="mono" value={renderText} onChange={(e) => setRenderText(e.target.value)} />
            </Field>
            <div className="mt-4">
              <Toggle
                checked={d.alwaysCreateRenderFolder}
                onChange={(v) => up("alwaysCreateRenderFolder", v)}
                label="Crear siempre «2. video/Renders finales»"
                hint="Por defecto solo se crea si hay renders identificados."
              />
            </div>
          </Section>

          <Section id="duplicados" title="Duplicados" hint="Nunca se sobrescribe un archivo existente.">
            <div className="space-y-4">
              <Toggle checked={d.duplicates.detectContent} onChange={(v) => up("duplicates", { ...d.duplicates, detectContent: v })} label="Detectar archivos idénticos con nombres diferentes (SHA-256)" hint="Usa tamaño y hash parcial antes de calcular el hash completo." />
              <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
                <Field label="Archivos idénticos con nombres o ubicaciones distintas">
                  <Select
                    value={d.duplicates.policy}
                    onChange={(v) => up("duplicates", { ...d.duplicates, policy: v })}
                    className="w-full"
                    options={[
                      { value: "copyAll", label: "Copiar todos (recomendado)" },
                      { value: "skipIdentical", label: "Copiar solo la primera aparición" },
                    ]}
                  />
                </Field>
                <Field label="Acción sugerida ante mismo nombre con contenido distinto">
                  <Select
                    value={d.duplicates.defaultConflictAction}
                    onChange={(v) => up("duplicates", { ...d.duplicates, defaultConflictAction: v })}
                    className="w-full"
                    options={[
                      { value: "ask", label: "Preguntar siempre" },
                      { value: "keepBoth", label: "Conservar ambos" },
                      { value: "skip", label: "Omitir" },
                    ]}
                  />
                </Field>
              </div>
            </div>
          </Section>

          <Section id="integridad" title="Verificación de integridad">
            <div className="space-y-4">
              <Field label="Método de verificación">
                <Select
                  value={d.copy.verify}
                  onChange={(v) => up("copy", { ...d.copy, verify: v })}
                  options={[
                    { value: "full", label: "SHA-256 completo: relee cada copia (recomendado)" },
                    { value: "size", label: "Solo tamaño: más rápido, menos seguro" },
                  ]}
                />
              </Field>
              <Toggle checked={d.copy.preserveMtime} onChange={(v) => up("copy", { ...d.copy, preserveMtime: v })} label="Conservar la fecha de modificación en las copias" />
              <Toggle
                checked={d.writeReportToDestination}
                onChange={(v) => up("writeReportToDestination", v)}
                label="Guardar reporte y mapa de rutas en el destino"
                hint="En «5. archivos/_Reporte Wonderlust» (o en la raíz en backups con jerarquía)."
              />
            </div>
          </Section>

          <Section id="rendimiento" title="Rendimiento" hint="Para discos duros mecánicos usa 1 copia simultánea; para SSD/NVMe, 2–4.">
            <div className="grid grid-cols-3 gap-4">
              <Field label="Copias simultáneas">
                <Input type="number" min={1} max={8} value={d.copy.concurrency} onChange={(e) => up("copy", { ...d.copy, concurrency: Number(e.target.value) || 1 })} />
              </Field>
              <Field label="Reintentos por archivo">
                <Input type="number" min={0} max={10} value={d.copy.retries} onChange={(e) => up("copy", { ...d.copy, retries: Number(e.target.value) || 0 })} />
              </Field>
              <Field label="Búfer de lectura (KB)">
                <Input type="number" min={256} max={32768} step={256} value={d.copy.bufferKb} onChange={(e) => up("copy", { ...d.copy, bufferKb: Number(e.target.value) || 4096 })} />
              </Field>
            </div>
          </Section>

          <Section id="rutas" title="Rutas frecuentes" hint="Aparecen como accesos rápidos al elegir un destino.">
            <div className="mb-4 space-y-1.5">
              {d.frequentPaths.length === 0 && <span className="text-xs text-faint">Sin rutas frecuentes.</span>}
              {d.frequentPaths.map((p, i) => (
                <div key={i} className="flex items-center gap-3 rounded-lg bg-bg px-3 py-2">
                  <span className="w-40 truncate text-[13px]">{p.label}</span>
                  <PathText path={p.path} className="flex-1" />
                  <button className="text-faint hover:text-danger" onClick={() => up("frequentPaths", d.frequentPaths.filter((_, j) => j !== i))}>
                    <Trash2 className="h-3.5 w-3.5" />
                  </button>
                </div>
              ))}
            </div>
            <div className="flex gap-2">
              <Input className="max-w-[200px]" placeholder="Nombre (p. ej. NAS)" value={newPath.label} onChange={(e) => setNewPath({ ...newPath, label: e.target.value })} />
              <Button
                onClick={async () => {
                  const r = await open({ directory: true, multiple: false });
                  if (typeof r === "string") setNewPath({ label: newPath.label || r.split(/[\\/]/).filter(Boolean).pop() || r, path: r });
                }}
              >
                {newPath.path ? "Cambiar carpeta" : "Elegir carpeta"}
              </Button>
              {newPath.path && <PathText path={newPath.path} className="flex-1 self-center" />}
              <Button
                icon={<Plus className="h-4 w-4" />}
                disabled={!newPath.path}
                onClick={() => {
                  up("frequentPaths", [...d.frequentPaths, newPath]);
                  setNewPath({ label: "", path: "" });
                }}
              >
                Añadir
              </Button>
            </div>
            {d.recentDestinations.length > 0 && (
              <div className="mt-5">
                <div className="mb-1.5 flex items-center justify-between text-xs text-muted">
                  Destinos recientes
                  <Button size="sm" variant="ghost" onClick={() => up("recentDestinations", [])}>
                    Limpiar
                  </Button>
                </div>
                {d.recentDestinations.map((r) => (
                  <PathText key={r} path={r} className="py-0.5 text-2xs" />
                ))}
              </div>
            )}
          </Section>

          <Section id="drive" title="Google Drive">
            <div className="flex items-start gap-3">
              <Cloud className="mt-0.5 h-5 w-5 text-muted" />
              <div className="space-y-2 text-[13px]">
                <div className="flex items-center gap-2">
                  Integración directa con la API de Google Drive <Badge tone="warn">Fase 4 · pendiente</Badge>
                </div>
                <p className="text-xs leading-relaxed text-muted">
                  La carga directa (autenticación OAuth, selección de carpetas y carga reanudable) requiere que Wonderlust registre un ID de cliente OAuth en Google
                  Cloud. Mientras tanto, si usas <b className="text-fg">Google Drive para escritorio</b>, tu Drive aparece como una carpeta local y puedes elegirlo como
                  destino con todas las garantías de copia y verificación.
                </p>
                {drives.length > 0 ? (
                  <div className="space-y-1">
                    <div className="text-xs text-ok">Google Drive para escritorio detectado:</div>
                    {drives.map((g) => (
                      <PathText key={g.path} path={g.path} />
                    ))}
                  </div>
                ) : (
                  <p className="text-xs text-faint">No se detectó Google Drive para escritorio en este equipo.</p>
                )}
              </div>
            </div>
          </Section>
        </div>
      </div>
    </div>
  );
}
