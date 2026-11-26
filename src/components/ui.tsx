import { forwardRef, useEffect, type ButtonHTMLAttributes, type InputHTMLAttributes, type ReactNode } from "react";
import { Loader2, X } from "lucide-react";
import type { Category } from "../lib/types";
import { categoryLabel } from "../lib/format";

export function cx(...c: (string | false | null | undefined)[]) {
  return c.filter(Boolean).join(" ");
}

type Variant = "primary" | "secondary" | "ghost" | "danger" | "subtle";

export const Button = forwardRef<
  HTMLButtonElement,
  ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant; size?: "sm" | "md" | "lg"; loading?: boolean; icon?: ReactNode }
>(function Button({ variant = "secondary", size = "md", loading, icon, className, children, disabled, ...rest }, ref) {
  const base =
    "inline-flex items-center justify-center gap-2 rounded-lg font-medium transition-colors duration-150 disabled:opacity-40 disabled:pointer-events-none whitespace-nowrap";
  const sizes = { sm: "h-7 px-2.5 text-xs", md: "h-9 px-3.5 text-[13px]", lg: "h-11 px-5 text-sm" };
  const variants: Record<Variant, string> = {
    primary: "bg-accent text-onaccent hover:brightness-110 shadow-[0_1px_0_rgb(255_255_255/0.15)_inset]",
    secondary: "bg-raised text-fg border border-line hover:border-faint",
    ghost: "text-muted hover:text-fg hover:bg-raised",
    subtle: "bg-fg/5 text-fg hover:bg-fg/10",
    danger: "bg-danger/15 text-danger border border-danger/30 hover:bg-danger/25",
  };
  return (
    <button ref={ref} className={cx(base, sizes[size], variants[variant], className)} disabled={disabled || loading} {...rest}>
      {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : icon}
      {children}
    </button>
  );
});

export function Card({ className, children, ...rest }: { className?: string; children: ReactNode } & React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div className={cx("rounded-xl border border-line bg-panel", className)} {...rest}>
      {children}
    </div>
  );
}

export function SectionTitle({ children, hint, right }: { children: ReactNode; hint?: ReactNode; right?: ReactNode }) {
  return (
    <div className="mb-3 flex items-end justify-between gap-4">
      <div>
        <h3 className="text-[13px] font-semibold tracking-tight">{children}</h3>
        {hint && <p className="mt-0.5 text-xs text-muted">{hint}</p>}
      </div>
      {right}
    </div>
  );
}

export const Input = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(function Input({ className, ...rest }, ref) {
  return (
    <input
      ref={ref}
      className={cx(
        "h-9 w-full rounded-lg border border-line bg-bg px-3 text-[13px] text-fg placeholder:text-faint transition-colors focus:border-accent/60 focus:outline-none",
        className,
      )}
      {...rest}
    />
  );
});

export function Field({ label, hint, children, className }: { label: string; hint?: ReactNode; children: ReactNode; className?: string }) {
  return (
    <label className={cx("block", className)}>
      <span className="mb-1.5 block text-xs font-medium text-muted">{label}</span>
      {children}
      {hint && <span className="mt-1 block text-2xs text-faint">{hint}</span>}
    </label>
  );
}

export function Select<T extends string>({
  value,
  onChange,
  options,
  className,
  disabled,
}: {
  value: T;
  onChange: (v: T) => void;
  options: { value: T; label: string }[];
  className?: string;
  disabled?: boolean;
}) {
  return (
    <select
      value={value}
      disabled={disabled}
      onChange={(e) => onChange(e.target.value as T)}
      className={cx(
        "h-9 rounded-lg border border-line bg-bg px-2.5 text-[13px] text-fg focus:border-accent/60 focus:outline-none disabled:opacity-50",
        className,
      )}
    >
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
  hint,
  disabled,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label?: ReactNode;
  hint?: ReactNode;
  disabled?: boolean;
}) {
  return (
    <label className={cx("flex cursor-pointer items-start gap-3", disabled && "pointer-events-none opacity-50")}>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        className={cx(
          "relative mt-0.5 inline-flex h-[18px] w-8 shrink-0 items-center rounded-full transition-colors",
          checked ? "bg-accent" : "bg-line",
        )}
      >
        <span
          className={cx(
            "inline-block h-3.5 w-3.5 rounded-full bg-white shadow transition-transform",
            checked ? "translate-x-[16px]" : "translate-x-[2px]",
          )}
        />
      </button>
      {(label || hint) && (
        <span className="min-w-0">
          {label && <span className="block text-[13px] text-fg">{label}</span>}
          {hint && <span className="block text-xs text-muted">{hint}</span>}
        </span>
      )}
    </label>
  );
}

export function Checkbox({ checked, onChange, indeterminate }: { checked: boolean; onChange: (v: boolean) => void; indeterminate?: boolean }) {
  return (
    <input
      type="checkbox"
      checked={checked}
      ref={(el) => {
        if (el) el.indeterminate = !!indeterminate;
      }}
      onChange={(e) => onChange(e.target.checked)}
      onClick={(e) => e.stopPropagation()}
      className="h-3.5 w-3.5 cursor-pointer accent-[rgb(var(--accent))]"
    />
  );
}

export function Badge({ children, tone = "neutral", className }: { children: ReactNode; tone?: "neutral" | "ok" | "warn" | "danger" | "accent"; className?: string }) {
  const tones = {
    neutral: "bg-fg/5 text-muted border-line",
    ok: "bg-ok/10 text-ok border-ok/25",
    warn: "bg-warn/10 text-warn border-warn/25",
    danger: "bg-danger/10 text-danger border-danger/25",
    accent: "bg-accent/10 text-accent border-accent/25",
  };
  return (
    <span className={cx("inline-flex h-5 items-center gap-1 whitespace-nowrap rounded-md border px-1.5 text-2xs font-medium", tones[tone], className)}>
      {children}
    </span>
  );
}

const CAT_CLASSES: Record<Category, string> = {
  imagen: "bg-cat-imagen/12 text-cat-imagen border-cat-imagen/25",
  video: "bg-cat-video/12 text-cat-video border-cat-video/25",
  audio: "bg-cat-audio/12 text-cat-audio border-cat-audio/25",
  editable: "bg-cat-editable/12 text-cat-editable border-cat-editable/25",
  archivos: "bg-cat-archivos/12 text-cat-archivos border-cat-archivos/25",
};

export const CAT_DOT: Record<Category, string> = {
  imagen: "bg-cat-imagen",
  video: "bg-cat-video",
  audio: "bg-cat-audio",
  editable: "bg-cat-editable",
  archivos: "bg-cat-archivos",
};

export function CategoryBadge({ category }: { category: Category }) {
  return (
    <span className={cx("inline-flex h-5 items-center rounded-md border px-1.5 text-2xs font-medium", CAT_CLASSES[category])}>
      {categoryLabel(category)}
    </span>
  );
}

export function ProgressBar({ value, tone = "accent", animated, className }: { value: number; tone?: "accent" | "ok" | "warn" | "danger"; animated?: boolean; className?: string }) {
  const tones = { accent: "bg-accent", ok: "bg-ok", warn: "bg-warn", danger: "bg-danger" };
  return (
    <div className={cx("h-1.5 w-full overflow-hidden rounded-full bg-line/70", className)}>
      <div
        className={cx("h-full rounded-full transition-[width] duration-300 ease-out", tones[tone], animated && "stripes")}
        style={{ width: `${Math.max(0, Math.min(100, value))}%` }}
      />
    </div>
  );
}

export function Spinner({ className }: { className?: string }) {
  return <Loader2 className={cx("h-4 w-4 animate-spin text-muted", className)} />;
}

export function Tabs<T extends string>({
  value,
  onChange,
  tabs,
}: {
  value: T;
  onChange: (v: T) => void;
  tabs: { value: T; label: string; count?: number; tone?: "warn" | "danger" }[];
}) {
  return (
    <div className="flex gap-1 border-b border-line">
      {tabs.map((t) => (
        <button
          key={t.value}
          onClick={() => onChange(t.value)}
          className={cx(
            "relative -mb-px flex h-9 items-center gap-1.5 border-b-2 px-3 text-[13px] transition-colors",
            value === t.value ? "border-accent text-fg" : "border-transparent text-muted hover:text-fg",
          )}
        >
          {t.label}
          {t.count != null && t.count > 0 && (
            <span
              className={cx(
                "rounded px-1 text-2xs tabular-nums",
                t.tone === "danger" ? "bg-danger/15 text-danger" : t.tone === "warn" ? "bg-warn/15 text-warn" : "bg-fg/5 text-muted",
              )}
            >
              {t.count.toLocaleString("es-ES")}
            </span>
          )}
        </button>
      ))}
    </div>
  );
}

export function Modal({
  open,
  onClose,
  title,
  children,
  footer,
  width = "max-w-lg",
}: {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  width?: string;
}) {
  useEffect(() => {
    if (!open) return;
    const h = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [open, onClose]);
  if (!open) return null;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/55 p-6 backdrop-blur-[2px]" onMouseDown={onClose}>
      <div
        className={cx("w-full animate-in rounded-2xl border border-line bg-panel shadow-2xl", width)}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-line px-5 py-3.5">
          <h2 className="text-sm font-semibold">{title}</h2>
          <button onClick={onClose} className="rounded p-1 text-muted hover:bg-raised hover:text-fg" aria-label="Cerrar">
            <X className="h-4 w-4" />
          </button>
        </div>
        <div className="max-h-[70vh] overflow-auto px-5 py-4">{children}</div>
        {footer && <div className="flex justify-end gap-2 border-t border-line px-5 py-3">{footer}</div>}
      </div>
    </div>
  );
}

export function Empty({ icon, title, children }: { icon?: ReactNode; title: string; children?: ReactNode }) {
  return (
    <div className="flex flex-col items-center justify-center px-6 py-12 text-center">
      {icon && <div className="mb-3 text-faint">{icon}</div>}
      <p className="text-[13px] font-medium">{title}</p>
      {children && <div className="mt-1 max-w-md text-xs text-muted">{children}</div>}
    </div>
  );
}

export function Stat({ label, value, sub, tone }: { label: string; value: ReactNode; sub?: ReactNode; tone?: "ok" | "warn" | "danger" | "accent" }) {
  const tones = { ok: "text-ok", warn: "text-warn", danger: "text-danger", accent: "text-accent" };
  return (
    <div className="min-w-0">
      <div className="text-2xs uppercase tracking-wider text-faint">{label}</div>
      <div className={cx("mt-1 truncate text-lg font-semibold tabular-nums tracking-tight", tone && tones[tone])}>{value}</div>
      {sub && <div className="truncate text-xs text-muted">{sub}</div>}
    </div>
  );
}

export function PathText({ path, className }: { path: string; className?: string }) {
  return (
    <span className={cx("selectable mono block truncate text-muted", className)} title={path} dir="ltr">
      {path}
    </span>
  );
}
