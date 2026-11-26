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
}

let toastId = 1;

export const useStore = create<AppStore>((set, get) => ({
  route: "home",
  settings: null,
  toasts: [],
  organize: emptyOrganize(),
  reportJobId: null,
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
}));

/** Aplica el tema visual al documento. */
export function applyTheme(theme: AppSettings["theme"] | undefined) {
  const root = document.documentElement;
  const prefersLight = window.matchMedia?.("(prefers-color-scheme: light)").matches;
  const light = theme === "light" || (theme === "system" && prefersLight);
  root.classList.toggle("light", !!light);
  root.classList.toggle("dark", !light);
}
