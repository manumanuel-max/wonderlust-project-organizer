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
write(path.join(SRC, "Footage/Día 2/Master grande.mov"), rnd(700 * 1024 * 1024, 3));
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

const driver = await new Builder()
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
  await driver.manage().window().setRect({ width: 1400, height: 880, x: 0, y: 0 }).catch(() => {});
  await waitText("Operaciones recientes");
  await sleep(400);
  await shot("01-inicio");

  // ---- Crear estructura ----
  await click("Crear estructura");
  await waitText("Vista previa");
  await typeIn("Banco Popular", "Banco Popular");
  await typeIn("Campaña Navidad", "Campaña Navidad");
  await sleep(500);
  await shot("02-crear-estructura");
  await click("Crear estructura");
  await waitText("Estructura lista");
  await shot("03-estructura-creada");
  const created = path.join(DEST, "25.11.26 BANCO POPULAR - CAMPAÑA NAVIDAD");
  check(["1. imagen", "2. video", "3. audio", "4. editable", "5. archivos"].every((f) => fs.statSync(path.join(created, f)).isDirectory()), "las cinco carpetas existen");

  // ---- Organizar ----
  await click("Organizar proyecto");
  await waitText("Contenido a organizar");
  // Simula el evento de soltar archivos del sistema operativo (el resto del flujo es real).
  // El listener de soltar se registra de forma asíncrona: reintentar hasta que la fuente aparezca.
  for (let i = 0; i < 20; i++) {
    await driver.executeScript(
      `return window.__TAURI_INTERNALS__.invoke('plugin:event|emit_to', { target: { kind: 'Webview', label: 'main' }, event: 'tauri://drag-drop', payload: { paths: [arguments[0]], position: { x: 10, y: 10 } } })`,
      SRC,
    );
    await sleep(400);
    if ((await driver.findElements(text("Quitar todo"))).length) break;
  }
  await waitText("Quitar todo");
  await sleep(600);
  // Los datos se autocompletan desde el nombre de la carpeta; se cambia el proyecto.
  const projInput = await driver.findElement(By.css('input[placeholder="Campaña Navidad"]'));
  check((await projInput.getAttribute("value")) === "CAMPAÑA NAVIDAD", "autocompletado desde la nomenclatura");
  await projInput.clear();
  await projInput.sendKeys("Backup Navidad");
  await sleep(500);
  await shot("04-organizar-seleccion");
  await click("Analizar contenido");
  await waitText("Estructura de destino", 60000);
  await sleep(800);
  // Expandir algunas carpetas del árbol.
  for (const n of ["2. video", "3. audio", "1. imagen"]) {
    const el = await driver.findElement(By.xpath(`//div[contains(@class,'select-none')]//span[normalize-space(.)=${JSON.stringify(n)}]`));
    await el.click();
    await sleep(300);
  }
  await shot("05-vista-previa-arbol");
  await click("Secuencias");
  await sleep(400);
  await shot("06-secuencias");
  await click("Todas independientes");
  await sleep(600);
  await click("Carpetas detectadas");
  await sleep(400);
  await shot("07-carpetas-detectadas");
  await click("Archivos");
  await sleep(700);
  await shot("08-archivos");
  await click("Exclusiones");
  await sleep(400);
  await shot("09-exclusiones");

  await click("Verificar destino");
  await waitText("Conflictos de nombre", 60000);
  await sleep(500);
  await shot("10-verificacion");
  // Resolver el conflicto: conservar ambos.
  const sel = await driver.findElement(By.xpath(`//select[option[@value='keepBoth']]`));
  await sel.click();
  await (await sel.findElement(By.css("option[value='keepBoth']"))).click();
  await sleep(600);
  await shot("11-conflicto-resuelto");
  await click("Confirmar y copiar");
  await waitText("Confirmar copia");
  const ack = await driver.findElement(By.xpath(`//label[contains(., 'backup organizado de archivos')]//input[@type='checkbox']`));
  await ack.click();
  await sleep(200);
  await shot("12-confirmacion");
  await click("Iniciar copia");
  await waitText("Archivos en curso", 20000);
  await sleep(700);
  await shot("13-copia-en-curso");
  // Pausa y reanudación desde la UI.
  await click("Pausar");
  await waitText("En pausa", 10000);
  await shot("14-copia-pausada");
  await sleep(800);
  await click("Reanudar");
  await waitText("Backup completado y verificado", 240000);
  await sleep(600);
  await shot("15-reporte");

  // ---- Verificación del sistema de archivos ----
  const out = path.join(DEST, PROJECT_NAME);
  const exists = (p) => fs.existsSync(path.join(out, p));
  check(exists("2. video/Footage/Día 1/A001_C001.MOV"), "video conserva subcarpetas funcionales");
  check(exists("2. video/Renders finales/Spot 30s FINAL.mp4"), "render final detectado");
  check(exists("2. video/Tarjeta B/PRIVATE/M4ROOT/CLIP/C0001M01.XML"), "tarjeta de cámara conservada completa");
  check(exists("3. audio/Música/Tema principal.wav") && exists("3. audio/Locuciones/VO toma 3.wav"), "música y locuciones");
  check(exists("3. audio/SFX/whoosh 01_copia1.wav"), "conflicto: se conservaron ambos");
  check(fs.readFileSync(path.join(out, "3. audio/SFX/whoosh 01.wav"), "utf8") === "contenido previo distinto", "archivo existente no sobrescrito");
  check(exists("4. editable/Proyecto Campaña ñ.aep") && exists("4. editable/Edición final.prproj"), "editables");
  check(fs.readdirSync(path.join(out, "4. editable")).every((n) => /\.(aep|aepx|prproj)$/i.test(n)), "editable solo contiene AEP/AEPX/PRPROJ");
  check(exists("1. imagen/Explosion/Explosion_0048.png"), "secuencia en su carpeta");
  check(exists("1. imagen/Humo/humo.0120.exr"), "secuencia incompleta copiada tal cual");
  check(exists("5. archivos/Diseño/Key visual.psd") && exists("5. archivos/notas.xyz"), "archivos y extensiones desconocidas");
  check(!exists("4. editable/Auto-Save") && !exists(".DS_Store"), "exclusiones aplicadas");
  const reports = path.join(out, "5. archivos/_Reporte Wonderlust");
  check(fs.existsSync(reports) && fs.readdirSync(reports).some((n) => n.startsWith("mapa_de_rutas")), "reporte y mapa de rutas en destino");
  const big1 = crypto.createHash("sha256").update(fs.readFileSync(path.join(SRC, "Footage/Día 2/Master grande.mov"))).digest("hex");
  const big2 = crypto.createHash("sha256").update(fs.readFileSync(path.join(out, "2. video/Renders finales/Master grande.mov"))).digest("hex");
  check(big1 === big2, "archivo grande idéntico (SHA-256)");
  const after = fingerprint(path.join(ROOT, "src"));
  check(JSON.stringify(before) === JSON.stringify(after), "originales intactos (nombre, tamaño, fecha y contenido)");
  const leftovers = execSync(`find ${JSON.stringify(DEST)} -name '*.wlpart' | wc -l`).toString().trim();
  check(leftovers === "0", "sin temporales en destino");

  // ---- Historial y configuración ----
  await click("Historial");
  await waitText("Registro local");
  await sleep(500);
  await shot("16-historial");
  await click("Configuración");
  await waitText("Reglas de clasificación");
  await sleep(300);
  await shot("17-configuracion");
  await click("Claro");
  await sleep(300);
  await click("Inicio");
  await waitText("Operaciones recientes");
  await sleep(500);
  await shot("18-inicio-tema-claro");
} catch (e) {
  failures++;
  console.error("ERROR:", e.message);
  await shot("error").catch(() => {});
} finally {
  await driver.quit().catch(() => {});
  tdriver.kill();
  xvfb.kill();
}
console.log(failures ? `\n${failures} fallo(s)` : "\nE2E OK");
process.exit(failures ? 1 : 0);
