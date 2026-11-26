import { open } from "@tauri-apps/plugin-dialog";
import { Cloud, FolderOpen, HardDrive, History, Home, Star } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../lib/api";
import { formatSize } from "../lib/format";
import { useStore } from "../lib/store";
import type { Location } from "../lib/types";
import { Button, cx, PathText } from "./ui";

const ICONS = {
  user: <Home className="h-3.5 w-3.5" />,
  volume: <HardDrive className="h-3.5 w-3.5" />,
  gdrive: <Cloud className="h-3.5 w-3.5" />,
  recent: <History className="h-3.5 w-3.5" />,
  frequent: <Star className="h-3.5 w-3.5" />,
};

/** Selector de carpeta de destino con accesos a ubicaciones frecuentes. */
export function DestinationPicker({ value, onChange, title = "Ubicación de destino" }: { value: string; onChange: (p: string) => void; title?: string }) {
  const [locations, setLocations] = useState<Location[]>([]);
  const [free, setFree] = useState<number | null>(null);
  const toast = useStore((s) => s.toast);

  useEffect(() => {
    api.listLocations().then(setLocations).catch(() => {});
  }, []);

  useEffect(() => {
    if (!value) return setFree(null);
    api
      .pathInfo(value)
      .then((i) => setFree(i.exists ? i.freeBytes : null))
      .catch(() => setFree(null));
  }, [value]);

  const choose = async () => {
    try {
      const r = await open({ directory: true, multiple: false, title, defaultPath: value || undefined });
      if (typeof r === "string") onChange(r);
    } catch (e) {
      toast("error", String(e));
    }
  };

  const shown = locations.filter((l) => l.available && l.kind !== "user").slice(0, 10);
  const userDirs = locations.filter((l) => l.available && l.kind === "user");

  return (
    <div>
      <div className="flex items-center gap-2">
        <button
          onClick={choose}
          className={cx(
            "flex h-10 min-w-0 flex-1 items-center gap-2.5 rounded-lg border border-line bg-bg px-3 text-left transition-colors hover:border-faint",
          )}
        >
          <FolderOpen className="h-4 w-4 shrink-0 text-muted" />
          {value ? <PathText path={value} className="!text-fg" /> : <span className="text-faint">Elegir carpeta…</span>}
          {free != null && <span className="ml-auto shrink-0 text-2xs text-muted">{formatSize(free)} libres</span>}
        </button>
        <Button onClick={choose}>Examinar</Button>
      </div>
      {(shown.length > 0 || userDirs.length > 0) && (
        <div className="mt-2 flex flex-wrap gap-1.5">
          {[...shown, ...userDirs].map((l) => (
            <button
              key={l.kind + l.path}
              onClick={() => onChange(l.path)}
              title={l.path}
              className={cx(
                "flex h-7 max-w-[220px] items-center gap-1.5 rounded-md border px-2 text-xs transition-colors",
                value === l.path ? "border-accent/50 bg-accent/10 text-fg" : "border-line text-muted hover:border-faint hover:text-fg",
              )}
            >
              <span className="shrink-0">{ICONS[l.kind]}</span>
              <span className="truncate">{l.label}</span>
              {l.freeBytes != null && l.kind !== "recent" && <span className="shrink-0 text-faint">· {formatSize(l.freeBytes)}</span>}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
