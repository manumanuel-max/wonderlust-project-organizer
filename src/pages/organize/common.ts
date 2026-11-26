import { api } from "../../lib/api";
import { joinPath } from "../../lib/format";
import type { AppSettings, OrganizeMode, PlanOptions } from "../../lib/types";

export function optionsFromSettings(s: AppSettings, mode: OrganizeMode): PlanOptions {
  return {
    mode,
    preserveSubfolders: s.preserveSubfolders,
    exclusions: s.exclusions,
    sequences: s.sequences,
    renderPatterns: s.renderPatterns,
    alwaysCreateRenderFolder: s.alwaysCreateRenderFolder,
    customRules: s.customRules,
  };
}

/** Nombre de la carpeta de proyecto en destino. */
export async function projectFolderName(date: string, client: string, project: string, rootName: string): Promise<string> {
  if (!client.trim() && !project.trim()) return rootName || "Proyecto organizado";
  const p = await api.previewProjectName(date, client, project);
  return p.name;
}

export async function destinationRoot(destParent: string, date: string, client: string, project: string, rootName: string) {
  return joinPath(destParent, await projectFolderName(date, client, project, rootName));
}
