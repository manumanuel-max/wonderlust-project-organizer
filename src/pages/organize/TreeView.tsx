import { ChevronRight, Clapperboard, File, Folder, FolderOpen } from "lucide-react";
import { useEffect, useState } from "react";
import { CAT_DOT, cx, Spinner } from "../../components/ui";
import { api } from "../../lib/api";
import { formatNumber, formatSize } from "../../lib/format";
import type { TreeNode } from "../../lib/types";

const MAX_FILES = 300;

function Node({ node, depth, selected, onSelect, version }: { node: TreeNode; depth: number; selected: string; onSelect: (p: string) => void; version: number }) {
  const [open, setOpen] = useState<boolean>(false);
  const [children, setChildren] = useState<TreeNode[] | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    setChildren(null);
  }, [version]);

  useEffect(() => {
    if (!open || children || !node.isDir) return;
    setLoading(true);
    api
      .treeChildren(node.path)
      .then(setChildren)
      .finally(() => setLoading(false));
  }, [open, children, node]);

  const isSel = selected === node.path;
  const dirs = children?.filter((c) => c.isDir) ?? [];
  const files = children?.filter((c) => !c.isDir) ?? [];
  const catKey = node.path.split("/")[0];
  const dot = { "1. imagen": CAT_DOT.imagen, "2. video": CAT_DOT.video, "3. audio": CAT_DOT.audio, "4. editable": CAT_DOT.editable, "5. archivos": CAT_DOT.archivos }[catKey];

  return (
    <div>
      <div
        className={cx("flex h-7 cursor-default items-center gap-1.5 rounded-md pr-2 text-[13px]", isSel ? "bg-accent/12 text-fg" : "hover:bg-raised/70")}
        style={{ paddingLeft: depth * 14 + 4 }}
        onClick={() => {
          if (node.isDir) {
            setOpen(!open);
            onSelect(node.path);
          }
        }}
      >
        {node.isDir ? (
          <ChevronRight className={cx("h-3.5 w-3.5 shrink-0 text-faint transition-transform", open && "rotate-90")} />
        ) : (
          <span className="w-3.5" />
        )}
        {node.isDir ? (
          open ? <FolderOpen className="h-3.5 w-3.5 shrink-0 text-muted" /> : <Folder className="h-3.5 w-3.5 shrink-0 text-muted" />
        ) : node.render ? (
          <Clapperboard className="h-3.5 w-3.5 shrink-0 text-accent" />
        ) : (
          <File className="h-3.5 w-3.5 shrink-0 text-faint" />
        )}
        {depth === 0 && dot && <span className={cx("h-1.5 w-1.5 shrink-0 rounded-full", dot)} />}
        <span className={cx("truncate", !node.isDir && "text-muted")}>{node.name}</span>
        <span className="ml-auto shrink-0 pl-2 text-2xs tabular-nums text-faint">
          {node.isDir ? (node.fileCount > 0 ? `${formatNumber(node.fileCount)} · ${formatSize(node.size)}` : "vacía") : formatSize(node.size)}
        </span>
      </div>
      {open && (
        <div>
          {loading && (
            <div style={{ paddingLeft: depth * 14 + 26 }} className="py-1">
              <Spinner className="h-3 w-3" />
            </div>
          )}
          {dirs.map((c) => (
            <Node key={c.path} node={c} depth={depth + 1} selected={selected} onSelect={onSelect} version={version} />
          ))}
          {files.slice(0, MAX_FILES).map((c) => (
            <Node key={c.path} node={c} depth={depth + 1} selected={selected} onSelect={onSelect} version={version} />
          ))}
          {files.length > MAX_FILES && (
            <div className="py-1 text-2xs text-faint" style={{ paddingLeft: depth * 14 + 46 }}>
              … y {formatNumber(files.length - MAX_FILES)} archivos más (usa la tabla para verlos)
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export function TreeView({ rootName, version, selected, onSelect }: { rootName: string; version: number; selected: string; onSelect: (p: string) => void }) {
  const [roots, setRoots] = useState<TreeNode[]>([]);
  useEffect(() => {
    api.treeChildren("").then(setRoots).catch(() => {});
  }, [version]);
  return (
    <div className="select-none">
      <div
        className={cx("flex h-7 items-center gap-1.5 rounded-md px-1 text-[13px] font-medium", selected === "" ? "bg-accent/12" : "hover:bg-raised/70")}
        onClick={() => onSelect("")}
      >
        <FolderOpen className="h-3.5 w-3.5 text-accent" />
        <span className="truncate">{rootName}</span>
      </div>
      {roots.map((n) => (
        <Node key={n.path} node={n} depth={1} selected={selected} onSelect={onSelect} version={version} />
      ))}
    </div>
  );
}
