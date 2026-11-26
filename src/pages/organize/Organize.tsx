import { Check } from "lucide-react";
import { PageHeader } from "../../components/Shell";
import { Button, cx } from "../../components/ui";
import { useStore, type OrganizeStep } from "../../lib/store";
import CopyStep from "./CopyStep";
import ReportStep from "./ReportView";
import ReviewStep from "./ReviewStep";
import SourcesStep from "./SourcesStep";
import VerifyStep from "./VerifyStep";

const STEPS: { key: OrganizeStep; label: string }[] = [
  { key: "sources", label: "Selección" },
  { key: "review", label: "Vista previa" },
  { key: "verify", label: "Verificación" },
  { key: "copy", label: "Copia" },
  { key: "report", label: "Reporte" },
];

export default function Organize() {
  const step = useStore((s) => s.organize.step);
  const reset = useStore((s) => s.resetOrganize);
  const idx = STEPS.findIndex((s) => s.key === step);
  return (
    <div className="flex h-full flex-col">
      <PageHeader
        title="Organizar proyecto"
        subtitle="Clasifica un proyecto existente en la estructura estandarizada, copiando o moviendo los archivos."
        right={
          (step === "report" || step === "sources" || step === "review" || step === "verify") && (
            <Button variant="ghost" size="sm" onClick={reset}>
              Nueva operación
            </Button>
          )
        }
      />
      <div className="flex items-center gap-1 px-8 pb-4">
        {STEPS.map((s, i) => (
          <div key={s.key} className="flex items-center gap-1">
            <div
              className={cx(
                "flex h-7 items-center gap-2 rounded-full pl-1 pr-3 text-xs",
                i === idx ? "bg-raised text-fg" : i < idx ? "text-muted" : "text-faint",
              )}
            >
              <span
                className={cx(
                  "flex h-5 w-5 items-center justify-center rounded-full text-2xs font-semibold",
                  i < idx ? "bg-ok/20 text-ok" : i === idx ? "bg-accent text-onaccent" : "bg-line text-faint",
                )}
              >
                {i < idx ? <Check className="h-3 w-3" /> : i + 1}
              </span>
              {s.label}
            </div>
            {i < STEPS.length - 1 && <div className={cx("h-px w-6", i < idx ? "bg-ok/40" : "bg-line")} />}
          </div>
        ))}
      </div>
      <div className="min-h-0 flex-1">
        {step === "sources" && <SourcesStep />}
        {step === "review" && <ReviewStep />}
        {step === "verify" && <VerifyStep />}
        {step === "copy" && <CopyStep />}
        {step === "report" && <ReportStep />}
      </div>
    </div>
  );
}
