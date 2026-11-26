import { CheckCircle2, FolderPlus, FolderTree, History, Home, Info, RefreshCw, Settings, X, XCircle } from "lucide-react";
import type { ReactNode } from "react";
import { useStore, type Route } from "../lib/store";
import { cx } from "./ui";
import logoSvg from "../assets/logo-horizontal.svg?raw";

const NAV: { route: Route; label: string; icon: ReactNode }[] = [
  { route: "home", label: "Inicio", icon: <Home className="h-4 w-4" /> },
  { route: "create", label: "Crear estructura", icon: <FolderPlus className="h-4 w-4" /> },
  { route: "organize", label: "Organizar proyecto", icon: <FolderTree className="h-4 w-4" /> },
  { route: "history", label: "Historial", icon: <History className="h-4 w-4" /> },
  { route: "settings", label: "Configuración", icon: <Settings className="h-4 w-4" /> },
];

/** Logotipo horizontal de Wonderlust (amarillo en tema oscuro, tinta en tema claro). */
export function Logo({ className }: { className?: string }) {
  return <span className={cx("block text-brand [&>svg]:h-full [&>svg]:w-auto", className)} dangerouslySetInnerHTML={{ __html: logoSvg }} />;
}

export function Sidebar({ busy }: { busy: boolean }) {
  const route = useStore((s) => s.route);
  const setRoute = useStore((s) => s.setRoute);
  const step = useStore((s) => s.organize.step);
  return (
    <aside className="flex w-[216px] shrink-0 flex-col border-r border-line bg-panel/60">
      <div className="px-4 pb-5 pt-5">
        <Logo className="h-9" />
        <div className="mt-1.5 pl-0.5 text-2xs uppercase tracking-[0.18em] text-faint">Project Organizer</div>
      </div>
      <nav className="flex flex-col gap-0.5 px-2">
        {NAV.map((n) => (
          <button
            key={n.route}
            onClick={() => setRoute(n.route)}
            className={cx(
              "flex h-9 items-center gap-2.5 rounded-lg px-2.5 text-[13px] transition-colors",
              route === n.route ? "bg-raised text-fg" : "text-muted hover:bg-raised/60 hover:text-fg",
            )}
          >
            <span className={route === n.route ? "text-accent" : ""}>{n.icon}</span>
            <span className="flex-1 text-left">{n.label}</span>
            {n.route === "organize" && busy && step === "copy" && (
              <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-accent" title="Copia en curso" />
            )}
          </button>
        ))}
      </nav>
      <button
        onClick={() => useStore.getState().requestUpdateCheck()}
        className="mx-2 mt-auto mb-2 flex items-center gap-2 rounded-lg px-2.5 py-2 text-left text-xs text-muted hover:bg-raised/60 hover:text-fg"
      >
        <RefreshCw className="h-3.5 w-3.5" /> Buscar actualizaciones
      </button>
      <div className="px-4 pb-4 text-2xs leading-relaxed text-faint">
        Por defecto solo copia. Si eliges mover, cada original se retira solo después de verificar su copia.
      </div>
    </aside>
  );
}

export function Toaster() {
  const toasts = useStore((s) => s.toasts);
  const dismiss = useStore((s) => s.dismissToast);
  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-[60] flex w-[380px] flex-col gap-2">
      {toasts.map((t) => (
        <div
          key={t.id}
          className="pointer-events-auto flex animate-in items-start gap-2.5 rounded-xl border border-line bg-raised px-3.5 py-3 shadow-xl"
        >
          {t.kind === "success" ? (
            <CheckCircle2 className="mt-px h-4 w-4 shrink-0 text-ok" />
          ) : t.kind === "error" ? (
            <XCircle className="mt-px h-4 w-4 shrink-0 text-danger" />
          ) : (
            <Info className="mt-px h-4 w-4 shrink-0 text-muted" />
          )}
          <p className="selectable flex-1 text-[13px] leading-snug">{t.text}</p>
          <button onClick={() => dismiss(t.id)} className="text-faint hover:text-fg" aria-label="Cerrar">
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      ))}
    </div>
  );
}

export function PageHeader({ title, subtitle, right }: { title: string; subtitle?: ReactNode; right?: ReactNode }) {
  return (
    <header className="flex items-end justify-between gap-6 px-8 pb-5 pt-7">
      <div>
        <h1 className="text-xl font-semibold tracking-tight">{title}</h1>
        {subtitle && <p className="mt-1 text-[13px] text-muted">{subtitle}</p>}
      </div>
      {right}
    </header>
  );
}
