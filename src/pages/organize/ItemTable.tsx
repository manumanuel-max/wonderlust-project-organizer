import { useVirtualizer } from "@tanstack/react-virtual";
import { Ban, CheckSquare, Clapperboard, FolderInput, RotateCcw } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Badge, Button, CategoryBadge, Checkbox, cx, Input, Modal, Select } from "../../components/ui";
import { api } from "../../lib/api";
import { CATEGORIES, formatNumber, formatSize } from "../../lib/format";
import { useStore } from "../../lib/store";
import type { Category, ItemFilter, ItemQuery, PlanItem, PlanResponse } from "../../lib/types";

const PAGE = 400;
const ROW = 46;

const EXCLUSION_LABELS: Record<string, string> = {
  aeAutosave: "Autosave AE",
  prAutosave: "Autosave PR",
  mediaCache: "Media Cache",
  aeCache: "Caché AE",
  renderPreviews: "Previews",
  proxy: "Proxy",
  temp: "Temporal",
  hidden: "Oculto",
  system: "Sistema",
  customExtension: "Extensión excluida",
  customFolder: "Carpeta excluida",
  manual: "Excluido manualmente",
};

export function ItemTable({
  base,
  version,
  onPlan,
  showFilters = true,
}: {
  base: ItemQuery;
  version: number;
  onPlan: (p: PlanResponse) => void;
  showFilters?: boolean;
}) {
  const toast = useStore((s) => s.toast);
  const [search, setSearch] = useState("");
  const [category, setCategory] = useState<Category | "all">("all");
  const [filter, setFilter] = useState<ItemFilter>("all");
  const [items, setItems] = useState<PlanItem[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [moveOpen, setMoveOpen] = useState(false);
  const [subfolder, setSubfolder] = useState("");
  const parentRef = useRef<HTMLDivElement>(null);
  const reqId = useRef(0);

  const query = useCallback(
    (offset: number): ItemQuery => ({
      ...base,
      search: search || base.search,
      category: category === "all" ? base.category ?? null : category,
      filter: filter === "all" ? base.filter ?? "all" : filter,
      offset,
      limit: PAGE,
    }),
    [base, search, category, filter],
  );

  // Recarga completa ante cambios de filtro o del plan.
  useEffect(() => {
    const id = ++reqId.current;
    setLoading(true);
    const t = setTimeout(() => {
      api
        .queryItems(query(0))
        .then((p) => {
          if (id !== reqId.current) return;
          setItems(p.items);
          setTotal(p.total);
        })
        .catch((e) => toast("error", e.message))
        .finally(() => id === reqId.current && setLoading(false));
    }, 120);
    return () => clearTimeout(t);
  }, [query, version, toast]);

  useEffect(() => setSelected(new Set()), [base, search, category, filter]);

  const v = useVirtualizer({ count: items.length, getScrollElement: () => parentRef.current, estimateSize: () => ROW, overscan: 12 });
  const vItems = v.getVirtualItems();

  // Carga incremental al acercarse al final.
  useEffect(() => {
    const last = vItems[vItems.length - 1];
    if (!last || loading || items.length >= total) return;
    if (last.index >= items.length - 40) {
      setLoading(true);
      const id = reqId.current;
      api
        .queryItems(query(items.length))
        .then((p) => id === reqId.current && setItems((prev) => [...prev, ...p.items]))
        .finally(() => setLoading(false));
    }
  }, [vItems, items.length, total, loading, query]);

  const ids = Array.from(selected);
  const apply = async (patch: Parameters<typeof api.setItemOverrides>[1]) => {
    try {
      const plan = await api.setItemOverrides(ids, patch);
      onPlan(plan);
    } catch (e) {
      toast("error", (e as Error).message);
    }
  };
  const allSelected = items.length > 0 && items.every((i) => selected.has(i.id));
  const someSelected = selected.size > 0;
  const editableBlocked = items.some((i) => selected.has(i.id) && i.category === "editable");

  return (
    <div className="flex h-full min-h-0 flex-col">
      {showFilters && (
        <div className="flex flex-wrap items-center gap-2 pb-3">
          <Input placeholder="Buscar por nombre o ruta…" value={search} onChange={(e) => setSearch(e.target.value)} className="h-8 max-w-[240px] flex-1" />
          <Select
            value={category}
            onChange={setCategory}
            className="h-8 text-xs"
            options={[{ value: "all", label: "Todas las categorías" }, ...CATEGORIES.map((c) => ({ value: c.key, label: c.folder }))]}
          />
          <Select
            value={filter}
            onChange={setFilter}
            className="h-8 text-xs"
            options={[
              { value: "all", label: "Todos" },
              { value: "included", label: "Incluidos" },
              { value: "excluded", label: "Excluidos" },
              { value: "render", label: "Renders finales" },
              { value: "sequence", label: "En secuencias" },
              { value: "unknown", label: "Sin clasificación específica" },
              { value: "adobe", label: "Proyectos de Adobe" },
              { value: "overridden", label: "Cambiados manualmente" },
            ]}
          />
          <span className="ml-auto text-xs text-muted">{formatNumber(total)} archivos</span>
        </div>
      )}

      {someSelected && (
        <div className="mb-2 flex flex-wrap items-center gap-2 rounded-lg border border-accent/30 bg-accent/[0.06] px-3 py-2">
          <span className="text-xs font-medium">{selected.size} seleccionado(s)</span>
          <span className="mx-1 h-4 w-px bg-line" />
          <Select
            value={"" as Category | ""}
            onChange={(c) => c && apply({ category: c as Category })}
            className="h-7 text-xs"
            disabled={editableBlocked}
            options={[
              { value: "", label: "Cambiar categoría…" },
              ...CATEGORIES.filter((c) => c.key !== "editable").map((c) => ({ value: c.key, label: c.folder })),
            ]}
          />
          <Button size="sm" variant="subtle" icon={<Clapperboard className="h-3.5 w-3.5" />} onClick={() => apply({ render: true })}>
            Marcar render final
          </Button>
          <Button size="sm" variant="subtle" onClick={() => apply({ render: false })}>
            Quitar render
          </Button>
          <Button size="sm" variant="subtle" icon={<FolderInput className="h-3.5 w-3.5" />} onClick={() => setMoveOpen(true)}>
            Subcarpeta…
          </Button>
          <Button size="sm" variant="subtle" icon={<Ban className="h-3.5 w-3.5" />} onClick={() => apply({ include: false })}>
            Excluir
          </Button>
          <Button size="sm" variant="subtle" icon={<CheckSquare className="h-3.5 w-3.5" />} onClick={() => apply({ include: true })}>
            Incluir
          </Button>
          <Button size="sm" variant="ghost" icon={<RotateCcw className="h-3.5 w-3.5" />} onClick={() => apply({ reset: true })}>
            Restablecer
          </Button>
          {editableBlocked && <span className="text-2xs text-muted">Los AEP/AEPX/PRPROJ siempre van a «4. editable».</span>}
        </div>
      )}

      <div className="flex h-8 shrink-0 items-center gap-3 border-b border-line px-3 text-2xs uppercase tracking-wider text-faint">
        <Checkbox
          checked={allSelected}
          indeterminate={someSelected && !allSelected}
          onChange={(v) => setSelected(v ? new Set(items.map((i) => i.id)) : new Set())}
        />
        <span className="flex-1">Origen → destino</span>
        <span className="w-[90px]">Categoría</span>
        <span className="w-[72px] text-right">Tamaño</span>
      </div>
      <div ref={parentRef} className="min-h-0 flex-1 overflow-auto">
        {items.length === 0 && !loading ? (
          <p className="py-10 text-center text-xs text-muted">No hay archivos con estos filtros.</p>
        ) : (
          <div style={{ height: v.getTotalSize(), position: "relative" }}>
            {vItems.map((row) => {
              const it = items[row.index];
              const sel = selected.has(it.id);
              return (
                <div
                  key={it.id}
                  onClick={() => {
                    const n = new Set(selected);
                    if (sel) n.delete(it.id);
                    else n.add(it.id);
                    setSelected(n);
                  }}
                  className={cx(
                    "absolute left-0 right-0 flex cursor-default items-center gap-3 border-b border-line/50 px-3",
                    sel ? "bg-accent/[0.07]" : "hover:bg-raised/60",
                    it.excluded && "opacity-55",
                  )}
                  style={{ height: ROW, transform: `translateY(${row.start}px)` }}
                >
                  <Checkbox checked={sel} onChange={() => {}} />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-1.5">
                      <span className="truncate text-[13px]" title={it.rel}>
                        {it.name}
                      </span>
                      {it.render && <Badge tone="accent">Render</Badge>}
                      {it.sequenceKey && <Badge>Secuencia</Badge>}
                      {it.overridden && <Badge tone="warn">Manual</Badge>}
                      {it.excluded && <Badge tone="danger">{EXCLUSION_LABELS[it.excluded] ?? "Excluido"}</Badge>}
                    </div>
                    <div className="mono truncate text-2xs text-faint" title={`${it.rel}  →  ${it.destRel}\n${it.reason}`}>
                      {it.excluded ? it.rel : `→ ${it.destRel}`}
                      <span className="ml-2 text-faint/80">· {it.reason}</span>
                    </div>
                  </div>
                  <span className="w-[90px]">
                    <CategoryBadge category={it.category} />
                  </span>
                  <span className="w-[72px] text-right text-xs tabular-nums text-muted">{formatSize(it.size)}</span>
                </div>
              );
            })}
          </div>
        )}
      </div>

      <Modal
        open={moveOpen}
        onClose={() => setMoveOpen(false)}
        title="Mover a una subcarpeta"
        footer={
          <>
            <Button variant="ghost" onClick={() => setMoveOpen(false)}>
              Cancelar
            </Button>
            <Button
              variant="primary"
              onClick={() => {
                apply({ subfolder });
                setMoveOpen(false);
              }}
            >
              Aplicar
            </Button>
          </>
        }
      >
        <p className="mb-3 text-xs text-muted">
          Subcarpeta dentro de la categoría de cada archivo (por ejemplo «Material/Día 1»). Déjala vacía para usar la ubicación automática.
        </p>
        <Input autoFocus value={subfolder} onChange={(e) => setSubfolder(e.target.value)} placeholder="Material/Día 1" />
      </Modal>
    </div>
  );
}
