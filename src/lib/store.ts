import { create } from "zustand";
import { api } from "./api";
import type { AnalysisResponse, AppSettings, OrganizeMode, PlanResponse } from "./types";
import { todayIso } from "./format";

export type Route = "home" | "create" | "organize" | "history" | "settings";
export type OrganizeStep = "sources" | "review" | "verify" | "copy" | "report";

export interface Toast {
  id: number;
  kind: "info" | "success" | "error";
  text: string;
}

interface OrganizeState {
  step: OrganizeStep;
  sources: string[];
  date: string;
  client: string;
  project: string;
  destParent: string;
  mode: OrganizeMode;
  plan: PlanResponse | null;
  analysis: AnalysisResponse | null;
  jobId: string | null;
}

const emptyOrganize = (mode: OrganizeMode = "organize"): OrganizeState => ({
  step: "sources",
  sources: [],
  date: todayIso(),
  client: "",
  project: "",
  destParent: "",
  mode,
  plan: null,
  analysis: null,
  jobId: null,
});

interface AppStore {
  route: Route;
  settings: AppSettings | null;
  toasts: Toast[];
  organize: OrganizeState;
  /** Pedir una búsqueda de actualizaciones al abrir Configuración. */
  checkUpdatesRequested: boolean;
  requestUpdateCheck: () => void;
  /** Reporte abierto desde el historial. */
  reportJobId: string | null;
  setRoute: (r: Route) => void;
  loadSettings: () => Promise<void>;
  setSettings: (s: AppSettings) => void;
  toast: (kind: Toast["kind"], text: string) => void;
  dismissToast: (id: number) => void;
  patchOrganize: (p: Partial<OrganizeState>) => void;
  resetOrganize: () => void;
  openReport: (jobId: string | null) => void;
  /** Carpetas recibidas desde el menú contextual del sistema. */
  openWithPaths: (paths: string[]) => Promise<void>;
}

let toastId = 1;

export const useStore = create<AppStore>((set, get) => ({
  route: "home",
  settings: null,
  toasts: [],
  organize: emptyOrganize(),
  reportJobId: null,
  checkUpdatesRequested: false,
  requestUpdateCheck: () => set({ checkUpdatesRequested: true, route: "settings" }),
  setRoute: (route) => set({ route }),
  loadSettings: async () => {
    const s = await api.getSettings();
    set({ settings: s });
    if (get().organize.step === "sources" && !get().organize.plan) {
      set({ organize: { ...get().organize, mode: s.defaultMode } });
    }
  },
  setSettings: (settings) => set({ settings }),
  toast: (kind, text) => {
    const id = toastId++;
    set({ toasts: [...get().toasts, { id, kind, text }] });
    setTimeout(() => get().dismissToast(id), kind === "error" ? 9000 : 4500);
  },
  dismissToast: (id) => set({ toasts: get().toasts.filter((t) => t.id !== id) }),
  patchOrganize: (p) => set({ organize: { ...get().organize, ...p } }),
  resetOrganize: () => {
    api.closeSession().catch(() => {});
    set({ organize: emptyOrganize(get().settings?.defaultMode ?? "organize") });
  },
  openReport: (jobId) => set({ reportJobId: jobId }),
  openWithPaths: async (paths) => {
    if (!paths.length) return;
    const o = get().organize;
    if (o.jobId && o.step === "copy") {
      get().toast("info", "Hay una operación en curso. Cuando termine, vuelve a usar «Organizar con Wonderlust».");
      set({ route: "organize" });
      return;
    }
    api.closeSession().catch(() => {});
    const base = emptyOrganize(get().settings?.defaultMode ?? "organize");
    const recent = get().settings?.recentDestinations[0] ?? "";
    let patch: Partial<OrganizeState> = { sources: Array.from(new Set(paths)), destParent: recent };
    if (paths.length === 1) {
      const name = paths[0].split(/[\\/]/).filter(Boolean).pop() ?? "";
      const parsed = await api.parseProjectName(name).catch(() => null);
      if (parsed) patch = { ...patch, date: parsed.date, client: parsed.client, project: parsed.project };
    }
    set({ organize: { ...base, ...patch }, route: "organize", reportJobId: null });
  },
}));

/** Aplica el tema visual al documento. */
export function applyTheme(theme: AppSettings["theme"] | undefined) {
  const root = document.documentElement;
  const prefersLight = window.matchMedia?.("(prefers-color-scheme: light)").matches;
  const light = theme === "light" || (theme === "system" && prefersLight);
  root.classList.toggle("light", !!light);
  root.classList.toggle("dark", !light);
}
