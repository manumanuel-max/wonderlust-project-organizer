import { useEffect, useState } from "react";
import { Sidebar, Toaster } from "./components/Shell";
import { api, events, isTauri } from "./lib/api";
import { applyTheme, useStore } from "./lib/store";
import CreateStructure from "./pages/CreateStructure";
import HistoryPage from "./pages/History";
import Home from "./pages/Home";
import Organize from "./pages/organize/Organize";
import SettingsPage from "./pages/Settings";
import { ReportModal } from "./pages/organize/ReportView";

export default function App() {
  const route = useStore((s) => s.route);
  const settings = useStore((s) => s.settings);
  const loadSettings = useStore((s) => s.loadSettings);
  const toast = useStore((s) => s.toast);
  const jobId = useStore((s) => s.organize.jobId);
  const step = useStore((s) => s.organize.step);
  const patch = useStore((s) => s.patchOrganize);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri()) {
      setError("Esta interfaz debe ejecutarse dentro de la aplicación de escritorio.");
      return;
    }
    loadSettings().catch((e) => setError(String(e)));
  }, [loadSettings]);

  useEffect(() => {
    applyTheme(settings?.theme);
    if (settings?.theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: light)");
    const h = () => applyTheme("system");
    mq.addEventListener("change", h);
    return () => mq.removeEventListener("change", h);
  }, [settings?.theme]);

  // Aviso global al terminar una copia (aunque el usuario esté en otra sección).
  useEffect(() => {
    if (!isTauri()) return;
    const un = events.onJobFinished((e) => {
      const s = useStore.getState();
      if (e.success) toast("success", "Copia completada y verificada.");
      else if (e.status === "cancelled") toast("info", "Copia cancelada. Puedes reanudarla desde Inicio o Historial.");
      else toast("error", "La copia terminó con errores o incompleta. Revisa el reporte.");
      if (s.organize.jobId === e.jobId) patch({ step: "report" });
    });
    return () => {
      un.then((f) => f());
    };
  }, [toast, patch]);

  // Evitar que soltar archivos fuera de la zona abra el archivo en la ventana.
  useEffect(() => {
    const prevent = (e: DragEvent) => e.preventDefault();
    window.addEventListener("dragover", prevent);
    window.addEventListener("drop", prevent);
    return () => {
      window.removeEventListener("dragover", prevent);
      window.removeEventListener("drop", prevent);
    };
  }, []);

  if (error) {
    return (
      <div className="flex h-full items-center justify-center p-10 text-center text-muted">
        <p>{error}</p>
      </div>
    );
  }
  if (!settings) return <div className="h-full bg-bg" />;

  const busy = !!jobId && step === "copy";
  return (
    <div className="flex h-full">
      <Sidebar busy={busy} />
      <main className="min-w-0 flex-1 overflow-hidden">
        {route === "home" && <Home />}
        {route === "create" && <CreateStructure />}
        {route === "organize" && <Organize />}
        {route === "history" && <HistoryPage />}
        {route === "settings" && <SettingsPage />}
      </main>
      <ReportModal />
      <Toaster />
    </div>
  );
}

// Exponer la API en desarrollo para depuración desde la consola.
if (import.meta.env.DEV) (window as unknown as { wpo: typeof api }).wpo = api;
