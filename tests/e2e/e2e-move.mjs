// Prueba de extremo a extremo de la aplicación real (UI + backend Rust) con WebDriver.
// Uso: node e2e.mjs <binario>
import { Builder, By, until } from "selenium-webdriver";
import { spawn, execSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const BIN = path.resolve(process.argv[2]);
const ROOT = "/tmp/claude-0/e2e";
const SRC = path.join(ROOT, "src", "25.11.26 BANCO POPULAR - CAMPAÑA NAVIDAD");
const DEST = path.join(ROOT, "dest");
const SHOTS = process.env.E2E_SHOTS || path.join(path.dirname(new URL(import.meta.url).pathname), "shots");
const CONF = path.join(os.homedir(), ".config", "com.wonderlust.projectorganizer");
const DATA = path.join(os.homedir(), ".local", "share", "com.wonderlust.projectorganizer");

const log = (...a) => console.log("•", ...a);
function write(p, data) {
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, data);
}
function rnd(n, seed) {
  const b = Buffer.alloc(n);
  let x = seed * 2654435761 + 1;
  for (let i = 0; i < n; i++) {
    x ^= x << 13;
    x ^= x >>> 17;
    x ^= x << 5;
    b[i] = x & 0xff;
  }
  return b;
}
function fingerprint(dir) {
  const out = {};
  const walk = (d) => {
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const p = path.join(d, e.name);
      if (e.isDirectory()) walk(p);
      else {
        const st = fs.statSync(p);
        out[p] = `${st.size}|${st.mtimeMs}|${crypto.createHash("sha256").update(fs.readFileSync(p)).digest("hex")}`;
      }
    }
  };
  walk(dir);
  return out;
}

// ---------- Datos de prueba ----------
fs.rmSync(ROOT, { recursive: true, force: true });
fs.rmSync(CONF, { recursive: true, force: true });
fs.rmSync(DATA, { recursive: true, force: true });
fs.mkdirSync(DEST, { recursive: true });
fs.mkdirSync(SHOTS, { recursive: true });
write(path.join(SRC, "Footage/Día 1/A001_C001.MOV"), rnd(3_000_000, 1));
write(path.join(SRC, "Footage/Día 1/A001_C002.mov"), rnd(2_500_000, 2));
write(path.join(SRC, "Footage/Día 2/Master grande.mov"), rnd(30 * 1024 * 1024, 3));
write(path.join(SRC, "Tarjeta B/PRIVATE/M4ROOT/CLIP/C0001.MP4"), rnd(800_000, 4));
write(path.join(SRC, "Tarjeta B/PRIVATE/M4ROOT/CLIP/C0001M01.XML"), "<xml/>");
write(path.join(SRC, "Audio/Música/Tema principal.wav"), rnd(400_000, 5));
write(path.join(SRC, "Audio/SFX/whoosh 01.wav"), rnd(40_000, 6));
write(path.join(SRC, "Audio/Locución/VO toma 3.wav"), rnd(90_000, 7));
write(path.join(SRC, "Proyecto Campaña ñ.aep"), "AEP");
write(path.join(SRC, "Edición final.prproj"), "PRPROJ");
write(path.join(SRC, "Diseño/Key visual.psd"), rnd(200_000, 8));
write(path.join(SRC, "Diseño/Logo.ai"), rnd(20_000, 9));
write(path.join(SRC, "Docs/Guion v3 (aprobado).pdf"), "%PDF");
write(path.join(SRC, "Export/Spot 30s FINAL.mp4"), rnd(1_200_000, 10));
for (let i = 1; i <= 48; i++) write(path.join(SRC, `Comp/Explosion/Explosion_${String(i).padStart(4, "0")}.png`), rnd(3000, 100 + i));
for (let i = 1; i <= 120; i++) if (i < 60 || i > 63) write(path.join(SRC, `Comp/Humo/humo.${String(i).padStart(4, "0")}.exr`), rnd(2000, 400 + i));
for (let i = 1; i <= 3; i++) write(path.join(SRC, `Graficos/logo_${i}.png`), rnd(5000, 300 + i));
write(path.join(SRC, "Graficos/duplicado.png"), rnd(5000, 301)); // idéntico a logo_1.png
write(path.join(SRC, "Adobe After Effects Auto-Save/Proyecto auto-save 1.aep"), "autosave");
write(path.join(SRC, ".DS_Store"), "junk");
write(path.join(SRC, "notas.xyz"), "desconocido");
// Conflicto: el destino ya contiene un archivo con el mismo nombre y otro contenido.
const PROJECT_NAME = "25.11.26 BANCO POPULAR - BACKUP NAVIDAD";
write(path.join(DEST, PROJECT_NAME, "3. audio/SFX/whoosh 01.wav"), "contenido previo distinto");
const before = fingerprint(path.join(ROOT, "src"));

// Configuración inicial: destino reciente y 1 copia simultánea.
write(
  path.join(CONF, "settings.json"),
  JSON.stringify({ recentDestinations: [DEST], copy: { concurrency: 1, retries: 2, bufferKb: 4096, verify: "full", preserveMtime: true } }),
);

// ---------- Entorno gráfico y WebDriver ----------
const xvfb = spawn("Xvfb", [":99", "-screen", "0", "1440x900x24"], { stdio: "ignore" });
await new Promise((r) => setTimeout(r, 800));
const tdriver = spawn("tauri-driver", [], { env: { ...process.env, DISPLAY: ":99" }, stdio: ["ignore", "pipe", "pipe"] });
tdriver.stderr.on("data", (d) => process.env.E2E_VERBOSE && process.stderr.write(d));
await new Promise((r) => setTimeout(r, 1500));

let driver = await new Builder()
  .usingServer("http://127.0.0.1:4444/")
  .withCapabilities({ browserName: "wry", "tauri:options": { application: BIN } })
  .build();

const shot = async (name) => {
  fs.writeFileSync(path.join(SHOTS, `${name}.png`), Buffer.from(await driver.takeScreenshot(), "base64"));
  log("captura", name);
};
const text = (t) => By.xpath(`//*[contains(normalize-space(.), ${JSON.stringify(t)}) and not(*[contains(normalize-space(.), ${JSON.stringify(t)})])]`);
const waitText = (t, ms = 20000) => driver.wait(until.elementLocated(text(t)), ms, `No apareció: ${t}`);
const button = (t) => By.xpath(`//button[contains(normalize-space(.), ${JSON.stringify(t)})]`);
const click = async (t) => {
  const els = await driver.findElements(button(t));
  for (const el of els.reverse()) {
    if ((await el.isDisplayed()) && (await el.isEnabled())) {
      await el.click();
      return;
    }
  }
  throw new Error(`Botón no disponible: ${t}`);
};
const typeIn = async (placeholder, value) => {
  const el = await driver.findElement(By.css(`input[placeholder="${placeholder}"]`));
  await el.clear();
  await el.sendKeys(value);
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let failures = 0;
const check = (cond, msg) => {
  if (cond) log("OK", msg);
  else {
    failures++;
    console.log("✗ FALLO", msg);
  }
};


try {
  await waitText("Operaciones recientes");
  await click("Organizar proyecto");
  await waitText("Contenido a organizar");
  for (let i = 0; i < 20; i++) {
    await driver.executeScript(
      `return window.__TAURI_INTERNALS__.invoke('plugin:event|emit_to', { target: { kind: 'Webview', label: 'main' }, event: 'tauri://drag-drop', payload: { paths: [arguments[0]], position: { x: 10, y: 10 } } })`,
      SRC,
    );
    await sleep(400);
    if ((await driver.findElements(text("Quitar todo"))).length) break;
  }
  await waitText("Quitar todo");
  const projInput = await driver.findElement(By.css('input[placeholder="Campaña Navidad"]'));
  await projInput.clear();
  await projInput.sendKeys("Backup Navidad");
  await click("Analizar contenido");
  await waitText("Estructura de destino", 60000);
  await click("Secuencias");
  await click("Todas independientes");
  await sleep(500);
  await click("Verificar destino");
  await waitText("Conflictos de nombre", 60000);
  const sel = await driver.findElement(By.xpath(`//select[option[@value='keepBoth']]`));
  await sel.click();
  await (await sel.findElement(By.css("option[value='keepBoth']"))).click();
  await sleep(500);
  await click("Continuar");
  await waitText("¿Quieres copiar o mover los archivos?");
  await sleep(300);
  await shot("m1-dialogo-copiar");
  // Elegir mover.
  await (await driver.findElement(By.xpath(`//button[.//text()[normalize-space()='Mover']]`))).click();
  await sleep(300);
  const moveBtn = await driver.findElement(button("Mover archivos"));
  check(!(await moveBtn.isEnabled()), "mover exige confirmación explícita");
  await (await driver.findElement(By.xpath(`//label[contains(., 'dejarán de estar en su ubicación original')]//input[@type='checkbox']`))).click();
  await (await driver.findElement(By.xpath(`//label[contains(., 'backup organizado de archivos')]//input[@type='checkbox']`))).click();
  await sleep(300);
  await shot("m2-dialogo-mover");
  await click("Mover archivos");
  await waitText("Movimiento completado y verificado", 240000);
  await sleep(600);
  await shot("m3-reporte-mover");

  const out = path.join(DEST, PROJECT_NAME);
  const exists = (p) => fs.existsSync(path.join(out, p));
  check(exists("2. video/Footage/Día 1/A001_C001.MOV") && exists("1. imagen/Explosion/Explosion_0048.png"), "archivos en la estructura nueva");
  check(!fs.existsSync(path.join(SRC, "Footage/Día 1/A001_C001.MOV")), "el original se movió (ya no está en el origen)");
  check(!fs.existsSync(path.join(SRC, "Footage")), "carpetas vacías retiradas del origen");
  check(fs.existsSync(SRC), "la carpeta seleccionada se conserva");
  check(fs.existsSync(path.join(SRC, ".DS_Store")) && fs.existsSync(path.join(SRC, "Adobe After Effects Auto-Save/Proyecto auto-save 1.aep")), "excluidos permanecen en el origen");
  check(fs.readFileSync(path.join(out, "3. audio/SFX/whoosh 01.wav"), "utf8") === "contenido previo distinto", "archivo existente en destino no sobrescrito");
  // Contenido movido idéntico al original.
  const moved = {};
  for (const [p, v] of Object.entries(before)) moved[p] = v.split("|")[2];
  const h = (p) => crypto.createHash("sha256").update(fs.readFileSync(p)).digest("hex");
  check(h(path.join(out, "2. video/Renders finales/Master grande.mov")) === moved[path.join(SRC, "Footage/Día 2/Master grande.mov")], "contenido íntegro tras mover");
  await click("Historial");
  await waitText("Registro local");
  await sleep(400);
  await shot("m4-historial");
} catch (e) {
  failures++;
  console.error("ERROR:", e.message);
  await shot("m-error").catch(() => {});
} finally {
  await driver.quit().catch(() => {});
  tdriver.kill();
  xvfb.kill();
}
console.log(failures ? `\n${failures} fallo(s)` : "\nE2E MOVER OK");
process.exit(failures ? 1 : 0);
