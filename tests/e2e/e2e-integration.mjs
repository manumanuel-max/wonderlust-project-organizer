// Prueba del menú contextual "Organizar con Wonderlust" (Linux: script de Nautilus)
// y de la llegada de carpetas con la app cerrada y con la app ya abierta.
import { Builder, By, until } from "selenium-webdriver";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const BIN = path.resolve(process.argv[2]);
const ROOT = "/tmp/claude-0/e2e-int";
const A = path.join(ROOT, "25.11.26 BANCO POPULAR - CAMPAÑA NAVIDAD");
const B = path.join(ROOT, "Proyecto viejo");
const SCRIPT = path.join(os.homedir(), ".local/share/nautilus/scripts/Organizar con Wonderlust");
const SHOTS = process.env.E2E_SHOTS || path.join(path.dirname(new URL(import.meta.url).pathname), "shots");
fs.rmSync(ROOT, { recursive: true, force: true });
fs.rmSync(path.join(os.homedir(), ".config/com.wonderlust.projectorganizer"), { recursive: true, force: true });
fs.rmSync(SCRIPT, { force: true });
for (const d of [A, B]) {
  fs.mkdirSync(path.join(d, "Footage"), { recursive: true });
  fs.writeFileSync(path.join(d, "Footage/clip.mov"), "x");
}
fs.mkdirSync(SHOTS, { recursive: true });
let failures = 0;
const check = (c, m) => (c ? console.log("• OK", m) : (failures++, console.log("✗ FALLO", m)));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const text = (t) => By.xpath(`//*[contains(normalize-space(.), ${JSON.stringify(t)}) and not(*[contains(normalize-space(.), ${JSON.stringify(t)})])]`);

const xvfb = spawn("Xvfb", [":99", "-screen", "0", "1440x900x24"], { stdio: "ignore" });
await sleep(800);
const td = spawn("tauri-driver", [], { env: { ...process.env, DISPLAY: ":99" }, stdio: "ignore" });
await sleep(1500);
// Arranque "desde el menú": la app recibe la carpeta como argumento.
const d = await new Builder()
  .usingServer("http://127.0.0.1:4444/")
  .withCapabilities({ browserName: "wry", "tauri:options": { application: BIN, args: [A] } })
  .build();
try {
  await d.wait(until.elementLocated(text("Contenido a organizar")), 20000);
  await d.wait(until.elementLocated(text("Quitar todo")), 10000);
  await sleep(600);
  const client = await d.findElement(By.css('input[placeholder="Banco Popular"]')).getAttribute("value");
  check(client === "BANCO POPULAR", "al abrir desde el menú, la carpeta llega cargada y se autocompletan los datos");
  fs.writeFileSync(path.join(SHOTS, "i1-abierta-desde-menu.png"), Buffer.from(await d.takeScreenshot(), "base64"));
  check(fs.existsSync(SCRIPT) && fs.readFileSync(SCRIPT, "utf8").includes(BIN), "opción del menú contextual instalada al iniciar");

  // Con la app ya abierta: el script del menú lanza otra instancia, que entrega la carpeta y se cierra.
  const r = spawnSync(SCRIPT, [B], { env: { ...process.env, DISPLAY: ":99" }, timeout: 20000 });
  check(r.status === 0, "la segunda instancia entrega la carpeta y termina");
  await d.wait(until.elementLocated(text("Proyecto viejo")), 15000);
  await sleep(500);
  const shown = await d.findElements(text(path.basename(A)));
  check(shown.length === 0, "la nueva carpeta reemplaza la selección anterior");
  fs.writeFileSync(path.join(SHOTS, "i2-segunda-carpeta.png"), Buffer.from(await d.takeScreenshot(), "base64"));

  // Configuración: desactivar y volver a activar.
  await (await d.findElement(By.xpath(`//nav//button[contains(., 'Configuración')]`))).click();
  await d.wait(until.elementLocated(text("Menú contextual y avisos")), 10000);
  const sw = await d.findElement(By.xpath(`//label[contains(., 'Organizar con Wonderlust')]//button[@role='switch']`));
  await sw.click();
  await sleep(800);
  check(!fs.existsSync(SCRIPT), "desactivar quita la opción del menú");
  await sw.click();
  await sleep(800);
  check(fs.existsSync(SCRIPT), "activar la vuelve a instalar");
  const save = await d.findElement(By.xpath(`//button[contains(., 'Guardar cambios')]`));
  check(!(await save.isEnabled()), "el interruptor se aplica al instante (sin cambios pendientes)");
  await (await d.findElement(By.xpath(`//button[contains(., 'Probar')]`))).click();
  await sleep(500);
  const errs = await d.findElements(text("Error"));
  check(errs.length === 0, "probar sonido no produce errores");
  await d.executeScript("document.getElementById('integracion')?.scrollIntoView()");
  await sleep(300);
  fs.writeFileSync(path.join(SHOTS, "i3-configuracion.png"), Buffer.from(await d.takeScreenshot(), "base64"));
} catch (e) {
  failures++;
  console.log("ERROR", e.message);
  fs.writeFileSync(path.join(SHOTS, "i-error.png"), Buffer.from(await d.takeScreenshot(), "base64"));
} finally {
  await d.quit().catch(() => {});
  td.kill();
  xvfb.kill();
}
console.log(failures ? `${failures} fallo(s)` : "E2E MENÚ CONTEXTUAL OK");
process.exit(failures ? 1 : 0);
