// Tipos que reflejan las estructuras serializadas por el núcleo en Rust.

export type Category = "imagen" | "video" | "audio" | "editable" | "archivos";
export type OrganizeMode = "organize" | "preserveHierarchy";

export interface CustomRule {
  extension: string;
  category: Category;
}

export interface ExclusionSettings {
  aeAutosave: boolean;
  prAutosave: boolean;
  mediaCache: boolean;
  aeCache: boolean;
  renderPreviews: boolean;
  proxies: boolean;
  tempFiles: boolean;
  hiddenFiles: boolean;
  systemFiles: boolean;
  emptyFolders: boolean;
  customExtensions: string[];
  customFolders: string[];
}

export interface SequenceSettings {
  enabled: boolean;
  minFrames: number;
  minDensity: number;
  cameraPrefixes: string[];
}

export interface PlanOptions {
  mode: OrganizeMode;
  preserveSubfolders: boolean;
  exclusions: ExclusionSettings;
  sequences: SequenceSettings;
  renderPatterns: string[];
  alwaysCreateRenderFolder: boolean;
  customRules: CustomRule[];
}

export type ExclusionReason =
  | "aeAutosave"
  | "prAutosave"
  | "mediaCache"
  | "aeCache"
  | "renderPreviews"
  | "proxy"
  | "temp"
  | "hidden"
  | "system"
  | "customExtension"
  | "customFolder"
  | "manual";

export interface PlanItem {
  id: number;
  source: string;
  rel: string;
  name: string;
  ext: string;
  size: number;
  mtime: number;
  category: Category;
  destRel: string;
  reason: string;
  sequenceKey: string | null;
  groupDir: string | null;
  render: boolean;
  renderSuggested: boolean;
  excluded: ExclusionReason | null;
  unknownExt: boolean;
  overridden: boolean;
}

export interface PlanSequence {
  key: string;
  relDir: string;
  prefix: string;
  ext: string;
  padding: number;
  first: number;
  last: number;
  count: number;
  missingCount: number;
  missingRanges: [number, number][];
  confidence: "high" | "ambiguous";
  ambiguity: string | null;
  folderName: string;
  ownsFolder: boolean;
  totalSize: number;
  decision: boolean | null;
  accepted: boolean;
  destDir: string;
}

export type GroupKind =
  | "cameraCard"
  | "organizedProject"
  | "proxies"
  | "renders"
  | "music"
  | "sfx"
  | "voiceover"
  | "stock"
  | "graphics"
  | "autosave"
  | "cache";

export interface FolderGroup {
  relDir: string;
  name: string;
  kind: GroupKind;
  category: Category | null;
  fileCount: number;
  size: number;
  detail: string;
  enabled: boolean;
}

export interface PlanWarning {
  code: string;
  level: "info" | "warning" | "critical";
  title: string;
  message: string;
}

export interface CategoryStat {
  count: number;
  size: number;
}

export interface ExclusionStat {
  reason: ExclusionReason | null;
  label: string;
  count: number;
  size: number;
}

export interface PlanSummary {
  mode: OrganizeMode;
  totalFiles: number;
  totalSize: number;
  includedFiles: number;
  includedSize: number;
  byCategory: Record<Category, CategoryStat>;
  excluded: ExclusionStat[];
  excludedFiles: number;
  excludedSize: number;
  sequences: number;
  sequencesPending: number;
  renders: number;
  unknownExt: number;
  adobeProjects: number;
  groups: number;
  alreadyOrganized: boolean;
  scanErrors: number;
  symlinks: number;
  emptyDirs: number;
}

export interface ParsedProjectName {
  date: string;
  client: string;
  project: string;
}

export interface PlanResponse {
  sessionId: string;
  rootName: string;
  singleRoot: boolean;
  sources: string[];
  options: PlanOptions;
  summary: PlanSummary;
  warnings: PlanWarning[];
  sequences: PlanSequence[];
  groups: FolderGroup[];
  dirsToCreate: string[];
  suggestedName: ParsedProjectName | null;
}

export type ItemFilter = "all" | "included" | "excluded" | "render" | "sequence" | "unknown" | "adobe" | "overridden";

export interface ItemQuery {
  category?: Category | null;
  filter?: ItemFilter;
  search?: string;
  destDir?: string | null;
  offset?: number;
  limit?: number;
}

export interface ItemPage {
  total: number;
  items: PlanItem[];
}

export interface TreeNode {
  name: string;
  path: string;
  isDir: boolean;
  fileCount: number;
  size: number;
  itemId: number | null;
  category: Category | null;
  render: boolean;
}

export interface FileStat {
  path: string;
  size: number;
  mtime: number;
}

export type Resolution = { type: "skip" } | { type: "keepBoth" } | { type: "rename"; name: string };

export type ConflictKind = "existingIdentical" | "existingDifferent" | "internalIdentical" | "internalDifferent";

export interface Conflict {
  id: number;
  kind: ConflictKind;
  itemId: number;
  name: string;
  otherItemId: number | null;
  destRel: string;
  source: FileStat;
  other: FileStat;
  comparison: string;
  needsDecision: boolean;
  resolution: Resolution | null;
}

export interface DuplicateGroup {
  hash: string;
  size: number;
  itemIds: number[];
  rels: string[];
}

export interface PreflightIssue {
  code: string;
  level: "blocking" | "warning" | "info";
  title: string;
  detail: string;
  paths: string[];
  count: number;
}

export interface PreflightReport {
  ok: boolean;
  issues: PreflightIssue[];
  requiredBytes: number;
  availableBytes: number | null;
  filesToCopy: number;
  filesSkipped: number;
  filesystem: string | null;
}

export interface AnalysisResponse {
  destRoot: string;
  duplicates: DuplicateGroup[];
  conflicts: Conflict[];
  unresolved: number;
  preflight: PreflightReport;
  hashedFiles: number;
  hashedBytes: number;
}

export interface ActiveFile {
  idx: number;
  name: string;
  done: number;
  total: number;
  phase: string;
}

export interface Progress {
  bytesDone: number;
  bytesTotal: number;
  filesDone: number;
  filesTotal: number;
  filesFailed: number;
  filesSkipped: number;
  speedBps: number;
  elapsedMs: number;
  etaMs: number | null;
  active: ActiveFile[];
  paused: boolean;
  finished: boolean;
}

export type ReportStatus = "completed" | "completedWithErrors" | "cancelled" | "aborted";

export interface ReportTotals {
  planned: number;
  copied: number;
  verified: number;
  skipped: number;
  duplicates: number;
  excluded: number;
  failed: number;
  pending: number;
  bytesCopied: number;
  bytesTransferredThisRun: number;
  bytesExcluded: number;
  bytesFailed: number;
}

export interface ReportEntry {
  idx: number;
  source: string;
  dest: string;
  destRel: string;
  size: number;
  status: "copiado" | "omitido" | "error" | "pendiente";
  detail: string;
  sha256: string;
  renamed: boolean;
}

export interface ExcludedEntry {
  itemId: number;
  relSrc: string;
  size: number;
  reason: string;
}

export interface JobReport {
  jobId: string;
  kind: string;
  kindLabel: string;
  client: string;
  project: string;
  destRoot: string;
  sources: string[];
  createdAt: string;
  finishedAt: string;
  durationMs: number;
  status: ReportStatus;
  statusLabel: string;
  success: boolean;
  integrityMode: "full" | "size";
  integrityMessage: string;
  totals: ReportTotals;
  abortedReason: string | null;
  excluded: ExcludedEntry[];
  warnings: string[];
}

export interface EntryPage {
  total: number;
  entries: ReportEntry[];
}

export interface NamingSettings {
  template: string;
  uppercase: boolean;
}

export interface CopySettings {
  concurrency: number;
  retries: number;
  bufferKb: number;
  verify: "full" | "size";
  preserveMtime: boolean;
}

export interface AppSettings {
  theme: "dark" | "light" | "system";
  naming: NamingSettings;
  customRules: CustomRule[];
  exclusions: ExclusionSettings;
  sequences: SequenceSettings;
  renderPatterns: string[];
  alwaysCreateRenderFolder: boolean;
  preserveSubfolders: boolean;
  defaultMode: OrganizeMode;
  duplicates: {
    detectContent: boolean;
    policy: "copyAll" | "skipIdentical";
    defaultConflictAction: "ask" | "skip" | "keepBoth";
  };
  copy: CopySettings;
  frequentPaths: { label: string; path: string }[];
  recentDestinations: string[];
  writeReportToDestination: boolean;
}

export interface HistoryEntry {
  id: string;
  date: string;
  client: string;
  project: string;
  kind: "structure" | "organize" | "backup";
  destination: string;
  files: number;
  size: number;
  status: string;
  jobId: string | null;
}

export interface InterruptedJob {
  jobId: string;
  createdAt: string;
  kind: string;
  client: string;
  project: string;
  destRoot: string;
  filesDone: number;
  filesTotal: number;
  bytesTotal: number;
  reason: string;
}

export interface Location {
  label: string;
  path: string;
  kind: "user" | "volume" | "gdrive" | "recent" | "frequent";
  freeBytes: number | null;
  totalBytes: number | null;
  available: boolean;
}

export interface StructureCheck {
  root: string;
  rootExists: boolean;
  parentExists: boolean;
  missingFolders: string[];
  nameProblems: string[];
}

export interface StructureResult {
  root: string;
  created: string[];
  existing: string[];
}

export interface JobStatus {
  running: boolean;
  progress: Progress | null;
  kind: string;
  client: string;
  project: string;
  destRoot: string;
}

export interface PathInfo {
  exists: boolean;
  isDir: boolean;
  freeBytes: number | null;
  filesystem: string | null;
}
