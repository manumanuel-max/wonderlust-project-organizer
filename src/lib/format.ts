import type { Category } from "./types";

export const CATEGORIES: { key: Category; folder: string; label: string; description: string }[] = [
  { key: "imagen", folder: "1. imagen", label: "Imagen", description: "Fotos, RAW, EXR, DPX y secuencias" },
  { key: "video", folder: "2. video", label: "Video", description: "Material de cámara, renders y proxies" },
  { key: "audio", folder: "3. audio", label: "Audio", description: "Música, efectos y locuciones" },
  { key: "editable", folder: "4. editable", label: "Editable", description: "Solo After Effects y Premiere Pro" },
  { key: "archivos", folder: "5. archivos", label: "Archivos", description: "Gráficos, documentos y otros" },
];

export const categoryLabel = (c: Category) => CATEGORIES.find((x) => x.key === c)?.label ?? c;
export const categoryFolder = (c: Category) => CATEGORIES.find((x) => x.key === c)?.folder ?? c;

const UNITS = ["B", "KB", "MB", "GB", "TB"];

/** Tamaño legible en formato español (coma decimal). */
export function formatSize(bytes: number | null | undefined): string {
  if (bytes == null || !isFinite(bytes)) return "—";
  if (bytes < 1024) return `${bytes} B`;
  let v = bytes;
  let u = 0;
  while (v >= 1024 && u < UNITS.length - 1) {
    v /= 1024;
    u++;
  }
  const digits = v >= 100 ? 0 : v >= 10 ? 1 : 2;
  return `${v.toFixed(digits).replace(".", ",")} ${UNITS[u]}`;
}

export function formatSpeed(bps: number): string {
  if (!bps || bps < 1) return "—";
  return `${formatSize(bps)}/s`;
}

export function formatDuration(ms: number | null | undefined): string {
  if (ms == null || !isFinite(ms)) return "—";
  const total = Math.max(0, Math.round(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) return `${h} h ${String(m).padStart(2, "0")} min`;
  if (m > 0) return `${m} min ${String(s).padStart(2, "0")} s`;
  return `${s} s`;
}

export function formatNumber(n: number): string {
  return n.toLocaleString("es-ES");
}

export function formatDate(iso: string): string {
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso;
  return d.toLocaleString("es-ES", { day: "2-digit", month: "short", year: "numeric", hour: "2-digit", minute: "2-digit" });
}

export function formatUnix(secs: number): string {
  if (!secs) return "—";
  return formatDate(new Date(secs * 1000).toISOString());
}

/** Fecha local de hoy en formato AAAA-MM-DD. */
export function todayIso(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

/** Une rutas respetando el separador del sistema que ya usa `base`. */
export function joinPath(base: string, name: string): string {
  if (!base) return name;
  const sep = base.includes("\\") && !base.includes("/") ? "\\" : "/";
  return base.endsWith(sep) ? base + name : base + sep + name;
}

export function basename(p: string): string {
  const parts = p.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? p;
}

export function percent(done: number, total: number): number {
  if (!total) return 0;
  return Math.min(100, Math.max(0, (done / total) * 100));
}

export const STATUS_LABELS: Record<string, string> = {
  completed: "Completado",
  completedWithErrors: "Con errores",
  cancelled: "Cancelado",
  aborted: "Detenido",
  created: "Creado",
  running: "En curso",
  interrupted: "Interrumpido",
};

export const KIND_LABELS: Record<string, string> = {
  structure: "Crear estructura",
  organize: "Organizar proyecto",
  backup: "Backup con jerarquía",
  move: "Mover proyecto",
};
