import { FileText, FolderOpen, Play, Search, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { PageHeader } from "../components/Shell";
import { Badge, Button, Card, Checkbox, Empty, Input, Modal, Select, Toggle } from "../components/ui";
import { api } from "../lib/api";
import { formatDate, formatNumber, formatSize, KIND_LABELS, STATUS_LABELS } from "../lib/format";
import { useStore } from "../lib/store";
import type { HistoryEntry } from "../lib/types";
import { statusTone } from "./Home";

export default function HistoryPage() {
  const toast = useStore((s) => s.toast);
  const openReport = useStore((s) => s.openReport);
  const setRoute = useStore((s) => s.setRoute);
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [search, setSearch] = useState("");
  const [kind, setKind] = useState("all");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [confirm, setConfirm] = useState(false);
  const [deleteReports, setDeleteReports] = useState(false);

  const load = () => api.historyList().then(setEntries).catch((e) => toast("error", e.message));
  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const shown = useMemo(() => {
    const q = search.toLowerCase();
    return entries.filter(
      (e) => (kind === "all" || e.kind === kind) && (!q || `${e.client} ${e.project} ${e.destination}`.toLowerCase().includes(q)),
    );
  }, [entries, search, kind]);

  const remove = async () => {
    try {
      const n = await api.historyDelete(Array.from(selected), deleteReports);
      toast("success", `${n} registro(s) eliminados del historial. Los archivos de los proyectos no se tocaron.`);
      setSelected(new Set());
      setConfirm(false);
      load();
    } catch (e) {
      toast("error", (e as Error).message);
    }
  };

  const resume = async (e: HistoryEntry) => {
    if (!e.jobId) return;
    try {
      await api.resumeInterruptedJob(e.jobId);
      useStore.getState().patchOrganize({ jobId: e.jobId, step: "copy", client: e.client, project: e.project });
      setRoute("organize");
    } catch (err) {
      toast("error", (err as Error).message);
    }
  };

  return (
    <div className="flex h-full flex-col">
      <PageHeader title="Historial" subtitle="Registro local de estructuras creadas, organizaciones y backups." />
      <div className="mx-auto flex min-h-0 w-full max-w-[1180px] flex-1 flex-col px-8 pb-8">
        <div className="mb-3 flex items-center gap-2">
          <div className="relative max-w-xs flex-1">
            <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-faint" />
            <Input className="pl-8" placeholder="Cliente, proyecto o ruta…" value={search} onChange={(e) => setSearch(e.target.value)} />
          </div>
          <Select
            value={kind}
            onChange={setKind}
            options={[
              { value: "all", label: "Todas las operaciones" },
              { value: "structure", label: "Crear estructura" },
              { value: "organize", label: "Organizar proyecto" },
              { value: "backup", label: "Backup con jerarquía" },
            ]}
          />
          <div className="ml-auto" />
          {selected.size > 0 && (
            <Button variant="danger" icon={<Trash2 className="h-4 w-4" />} onClick={() => setConfirm(true)}>
              Eliminar {selected.size} registro(s)
            </Button>
          )}
        </div>
        <Card className="min-h-0 flex-1 overflow-auto">
          {shown.length === 0 ? (
            <Empty icon={<FileText className="h-8 w-8" />} title="Sin registros">
              Las operaciones que realices aparecerán aquí.
            </Empty>
          ) : (
            <table className="w-full text-[13px]">
              <thead className="sticky top-0 z-10 bg-panel">
                <tr className="border-b border-line text-left text-2xs uppercase tracking-wider text-faint">
                  <th className="w-10 px-4 py-2.5">
                    <Checkbox
                      checked={shown.length > 0 && shown.every((e) => selected.has(e.id))}
                      onChange={(v) => setSelected(v ? new Set(shown.map((e) => e.id)) : new Set())}
                    />
                  </th>
                  <th className="px-2 py-2.5 font-medium">Fecha</th>
                  <th className="px-2 py-2.5 font-medium">Cliente / proyecto</th>
                  <th className="px-2 py-2.5 font-medium">Operación</th>
                  <th className="px-2 py-2.5 font-medium">Destino</th>
                  <th className="px-2 py-2.5 text-right font-medium">Archivos</th>
                  <th className="px-2 py-2.5 text-right font-medium">Tamaño</th>
                  <th className="px-2 py-2.5 font-medium">Estado</th>
                  <th className="px-4 py-2.5" />
                </tr>
              </thead>
              <tbody>
                {shown.map((e) => (
                  <tr key={e.id} className="border-b border-line/60 hover:bg-raised/50">
                    <td className="px-4 py-2.5">
                      <Checkbox
                        checked={selected.has(e.id)}
                        onChange={(v) => {
                          const n = new Set(selected);
                          if (v) n.add(e.id);
                          else n.delete(e.id);
                          setSelected(n);
                        }}
                      />
                    </td>
                    <td className="whitespace-nowrap px-2 py-2.5 text-muted">{formatDate(e.date)}</td>
                    <td className="max-w-[220px] px-2 py-2.5">
                      <div className="truncate">{e.client || "—"}</div>
                      <div className="truncate text-xs text-muted">{e.project}</div>
                    </td>
                    <td className="whitespace-nowrap px-2 py-2.5 text-muted">{KIND_LABELS[e.kind] ?? e.kind}</td>
                    <td className="max-w-[260px] px-2 py-2.5">
                      <span className="mono block truncate text-2xs text-muted" title={e.destination}>
                        {e.destination}
                      </span>
                    </td>
                    <td className="px-2 py-2.5 text-right tabular-nums text-muted">{e.files ? formatNumber(e.files) : "—"}</td>
                    <td className="px-2 py-2.5 text-right tabular-nums text-muted">{e.size ? formatSize(e.size) : "—"}</td>
                    <td className="px-2 py-2.5">
                      <Badge tone={statusTone(e.status)}>{STATUS_LABELS[e.status] ?? e.status}</Badge>
                    </td>
                    <td className="whitespace-nowrap px-4 py-2.5 text-right">
                      {(e.status === "cancelled" || e.status === "aborted" || e.status === "interrupted") && e.jobId && (
                        <Button size="sm" variant="ghost" icon={<Play className="h-3.5 w-3.5" />} onClick={() => resume(e)}>
                          Reanudar
                        </Button>
                      )}
                      {e.jobId && e.status !== "running" && e.status !== "interrupted" && (
                        <Button size="sm" variant="ghost" icon={<FileText className="h-3.5 w-3.5" />} onClick={() => openReport(e.jobId)}>
                          Reporte
                        </Button>
                      )}
                      <Button size="sm" variant="ghost" icon={<FolderOpen className="h-3.5 w-3.5" />} onClick={() => api.openPath(e.destination).catch((err) => toast("error", err.message))}>
                        Abrir
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </Card>
      </div>
      <Modal
        open={confirm}
        onClose={() => setConfirm(false)}
        title="Eliminar del historial"
        footer={
          <>
            <Button variant="ghost" onClick={() => setConfirm(false)}>
              Cancelar
            </Button>
            <Button variant="danger" onClick={remove}>
              Eliminar registros
            </Button>
          </>
        }
      >
        <p className="text-[13px] text-muted">
          Se eliminarán {selected.size} registro(s) del historial. <b className="text-fg">Los archivos y carpetas de los proyectos no se eliminan.</b>
        </p>
        <div className="mt-4">
          <Toggle checked={deleteReports} onChange={setDeleteReports} label="Eliminar también los reportes guardados en la aplicación" hint="Los reportes escritos en la carpeta del proyecto se conservan." />
        </div>
      </Modal>
    </div>
  );
}
