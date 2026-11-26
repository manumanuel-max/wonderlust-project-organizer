// Envoltorios tipados de los comandos Tauri.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AnalysisResponse,
  AppSettings,
  Conflict,
  EntryPage,
  HistoryEntry,
  InterruptedJob,
  ItemPage,
  ItemQuery,
  JobReport,
  JobStatus,
  Location,
  ParsedProjectName,
  PathInfo,
  PlanOptions,
  PlanResponse,
  Progress,
  ReportStatus,
  Resolution,
  StructureCheck,
  StructureResult,
  TreeNode,
  Category,
  TransferOptions,
} from "./types";

export const isTauri = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Normaliza los errores devueltos por Rust (cadenas) a `Error`. */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw new Error(typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e));
  }
}

export const api = {
  getSettings: () => call<AppSettings>("get_settings"),
  saveSettings: (settings: AppSettings) => call<AppSettings>("save_settings", { settings }),
  resetSettings: () => call<AppSettings>("reset_settings"),

  previewProjectName: (date: string, client: string, project: string) =>
    call<{ name: string; problems: string[] }>("preview_project_name", { date, client, project }),
  parseProjectName: (name: string) => call<ParsedProjectName | null>("parse_project_name", { name }),
  checkStructure: (parent: string, name: string) => call<StructureCheck>("check_structure", { parent, name }),
  createStructure: (p: { parent: string; name: string; client: string; project: string; completeExisting: boolean }) =>
    call<StructureResult>("create_structure", p),

  openPath: (path: string) => call<void>("open_path", { path }),
  revealPath: (path: string) => call<void>("reveal_path", { path }),
  pathInfo: (path: string) => call<PathInfo>("path_info", { path }),
  listLocations: () => call<Location[]>("list_locations"),

  scanSources: (paths: string[], options: PlanOptions) => call<PlanResponse>("scan_sources", { paths, options }),
  cancelScan: () => call<void>("cancel_scan"),
  closeSession: () => call<void>("close_session"),
  updatePlan: (options: PlanOptions) => call<PlanResponse>("update_plan", { options }),
  setItemOverrides: (
    ids: number[],
    patch: { category?: Category; render?: boolean; include?: boolean; subfolder?: string; reset?: boolean },
  ) => call<PlanResponse>("set_item_overrides", { ids, patch }),
  setSequenceDecision: (keys: string[], accept: boolean | null) =>
    call<PlanResponse>("set_sequence_decision", { keys, accept }),
  setGroupEnabled: (relDir: string, enabled: boolean) => call<PlanResponse>("set_group_enabled", { relDir, enabled }),
  queryItems: (query: ItemQuery) => call<ItemPage>("query_items", { query }),
  treeChildren: (dir: string) => call<TreeNode[]>("tree_children", { dir }),
  getPlanDetails: () => call<PlanResponse>("get_plan_details"),

  runAnalysis: (destRoot: string) => call<AnalysisResponse>("run_analysis", { destRoot }),
  cancelAnalysis: () => call<void>("cancel_analysis"),
  resolveConflicts: (items: { id: number; resolution: Resolution | null }[], applyToEquivalent: boolean) =>
    call<{ conflicts: Conflict[]; unresolved: number }>("resolve_conflicts", { items, applyToEquivalent }),

  startJob: (client: string, project: string, transfer: TransferOptions) =>
    call<string>("start_job", { params: { client, project, transfer } }),
  pauseJob: (jobId: string) => call<void>("pause_job", { jobId }),
  resumeJob: (jobId: string) => call<void>("resume_job", { jobId }),
  cancelJob: (jobId: string) => call<void>("cancel_job", { jobId }),
  jobStatus: (jobId: string) => call<JobStatus>("job_status", { jobId }),
  listInterruptedJobs: () => call<InterruptedJob[]>("list_interrupted_jobs"),
  resumeInterruptedJob: (jobId: string) => call<void>("resume_interrupted_job", { jobId }),
  discardInterruptedJob: (jobId: string) => call<void>("discard_interrupted_job", { jobId }),
  retryFailed: (jobId: string) => call<number>("retry_failed", { jobId }),
  getReport: (jobId: string) => call<JobReport>("get_report", { jobId }),
  reportEntries: (jobId: string, status: string | null, search: string, offset: number, limit: number) =>
    call<EntryPage>("report_entries", { jobId, status, search, offset, limit }),
  exportReport: (jobId: string, format: "csv" | "json" | "txt" | "pathmap", path: string) =>
    call<string>("export_report", { jobId, format, path }),

  updateConfig: () => call<{ configured: boolean; currentVersion: string; channel: string | null }>("update_config"),
  checkUpdate: () => call<{ version: string; currentVersion: string; date: string | null; notes: string | null } | null>("check_update"),
  installUpdate: () => call<void>("install_update"),
  restartApp: () => call<void>("restart_app"),

  takeLaunchPaths: () => call<string[]>("take_launch_paths"),
  contextMenuStatus: () => call<{ supported: boolean; installed: boolean; location: string; hint: string }>("context_menu_status"),
  setContextMenu: (enabled: boolean) =>
    call<{ supported: boolean; installed: boolean; location: string; hint: string }>("set_context_menu", { enabled }),

  historyList: () => call<HistoryEntry[]>("history_list"),
  historyDelete: (ids: string[], deleteReports: boolean) => call<number>("history_delete", { ids, deleteReports }),
};

export const events = {
  onOpenPaths: (cb: (paths: string[]) => void): Promise<UnlistenFn> => listen<string[]>("open-paths", (e) => cb(e.payload)),
  onUpdateProgress: (cb: (p: { downloaded: number; total: number | null }) => void): Promise<UnlistenFn> =>
    listen<{ downloaded: number; total: number | null }>("update-progress", (e) => cb(e.payload)),
  onScanProgress: (cb: (p: { files: number; bytes: number }) => void): Promise<UnlistenFn> =>
    listen<{ files: number; bytes: number }>("scan-progress", (e) => cb(e.payload)),
  onAnalysisProgress: (cb: (p: { phase: string; done: number; total: number }) => void): Promise<UnlistenFn> =>
    listen<{ phase: string; done: number; total: number }>("analysis-progress", (e) => cb(e.payload)),
  onJobProgress: (cb: (p: { jobId: string; progress: Progress }) => void): Promise<UnlistenFn> =>
    listen<{ jobId: string; progress: Progress }>("job-progress", (e) => cb(e.payload)),
  onJobFinished: (cb: (p: { jobId: string; status: ReportStatus; success: boolean }) => void): Promise<UnlistenFn> =>
    listen<{ jobId: string; status: ReportStatus; success: boolean }>("job-finished", (e) => cb(e.payload)),
};
