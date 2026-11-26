import { Volume2 } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../lib/api";
import { playFinishSound } from "../lib/sound";
import { useStore } from "../lib/store";
import { Button, PathText, Toggle } from "./ui";

type Status = { supported: boolean; installed: boolean; location: string; hint: string };

export function IntegrationPanel({
  finishSound,
  onFinishSound,
  onContextMenuChanged,
}: {
  finishSound: boolean;
  onFinishSound: (v: boolean) => void;
  onContextMenuChanged: (v: boolean) => void;
}) {
  const toast = useStore((s) => s.toast);
  const [status, setStatus] = useState<Status | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.contextMenuStatus().then(setStatus).catch(() => {});
  }, []);

  const toggleMenu = async (v: boolean) => {
    setBusy(true);
    try {
      setStatus(await api.setContextMenu(v));
      onContextMenuChanged(v);
      toast("success", v ? "Se añadió «Organizar con Wonderlust» al menú contextual." : "Se quitó la opción del menú contextual.");
    } catch (e) {
      toast("error", (e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-5">
      {status?.supported && (
        <div className="space-y-2">
          <Toggle
            checked={status.installed}
            disabled={busy}
            onChange={toggleMenu}
            label="«Organizar con Wonderlust» en el clic derecho de las carpetas"
            hint="Abre la app con la carpeta ya cargada en Organizar proyecto. No copia ni mueve nada hasta que confirmes."
          />
          <p className="pl-11 text-xs text-muted">{status.hint}</p>
          {status.installed && <PathText path={status.location} className="pl-11 text-2xs" />}
        </div>
      )}
      <div className="flex items-start gap-3">
        <div className="flex-1">
          <Toggle
            checked={finishSound}
            onChange={onFinishSound}
            label="Sonido de aviso al terminar"
            hint="Suena al finalizar una copia o un movimiento (un tono distinto si hubo errores) y la app se señala en el Dock o la barra de tareas."
          />
        </div>
        <Button size="sm" variant="ghost" icon={<Volume2 className="h-3.5 w-3.5" />} onClick={() => playFinishSound("success")}>
          Probar
        </Button>
      </div>
    </div>
  );
}
