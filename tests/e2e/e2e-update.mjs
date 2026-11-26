// Prueba real del actualizador: firma falsa (rechazada) y firma válida (instalada).
import { Builder, By, until } from "selenium-webdriver";
import { spawn } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";

const U = "/tmp/claude-0/upd";
const APP = path.join(U, "app/wonderlust-project-organizer");
const SHOTS = path.join(path.dirname(new URL(import.meta.url).pathname), "shots-upd");
fs.mkdirSync(SHOTS, { recursive: true });
const h = (p) => crypto.createHash("sha256").update(fs.readFileSync(p)).digest("hex");
let failures = 0;
const check = (c, m) => (c ? console.log("• OK", m) : (failures++, console.log("✗ FALLO", m)));

function latest(bin) {
  const sig = fs.readFileSync(path.join(U, "www", bin + ".sig"), "utf8");
  fs.writeFileSync(
    path.join(U, "www/latest.json"),
    JSON.stringify({
      version: "0.2.0",
      notes: "Prueba: nueva versión con mejoras.",
      pub_date: "2026-11-26T10:00:00Z",
      platforms: { "linux-x86_64": { signature: sig, url: `http://127.0.0.1:8765/${bin}` } },
    }),
  );
}

const server = spawn("python3", ["-m", "http.server", "8765", "--bind", "127.0.0.1", "--directory", path.join(U, "www")], { stdio: "ignore" });
const xvfb = spawn("Xvfb", [":99", "-screen", "0", "1440x900x24"], { stdio: "ignore" });
await new Promise((r) => setTimeout(r, 800));
const td = spawn("tauri-driver", [], { env: { ...process.env, DISPLAY: ":99" }, stdio: "ignore" });
await new Promise((r) => setTimeout(r, 1500));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const text = (t) => By.xpath(`//*[contains(normalize-space(.), ${JSON.stringify(t)}) and not(*[contains(normalize-space(.), ${JSON.stringify(t)})])]`);

async function session(bin, expectOk) {
  latest(bin);
  const before = h(APP);
  const d = await new Builder().usingServer("http://127.0.0.1:4444/").withCapabilities({ browserName: "wry", "tauri:options": { application: APP } }).build();
  try {
    await d.wait(until.elementLocated(text("Operaciones recientes")), 20000);
    await (await d.findElement(By.xpath(`//aside//button[contains(., 'Buscar actualizaciones')]`))).click();
    await d.wait(until.elementLocated(text("Nueva versión disponible")), 20000);
    await sleep(300);
    await fs.promises.writeFile(path.join(SHOTS, `${bin}-1.png`), Buffer.from(await d.takeScreenshot(), "base64"));
    await (await d.findElement(By.xpath(`//button[contains(., 'Descargar e instalar')]`))).click();
    if (expectOk) {
      await d.wait(until.elementLocated(text("Reiniciar para terminar")), 30000);
      await fs.promises.writeFile(path.join(SHOTS, `${bin}-2.png`), Buffer.from(await d.takeScreenshot(), "base64"));
      check(h(APP) === h(path.join(U, "www", bin)), "firma válida: la actualización se instaló (ejecutable reemplazado)");
    } else {
      await d.wait(until.elementLocated(text("firma no es válida")), 30000);
      await fs.promises.writeFile(path.join(SHOTS, `${bin}-2.png`), Buffer.from(await d.takeScreenshot(), "base64"));
      check(h(APP) === before, "firma falsa: rechazada y la app no se modificó");
    }
  } catch (e) {
    failures++;
    console.log("ERROR", e.message);
    await fs.promises.writeFile(path.join(SHOTS, `${bin}-error.png`), Buffer.from(await d.takeScreenshot(), "base64")).catch(() => {});
  } finally {
    await d.quit().catch(() => {});
  }
}

await session("falsa.bin", false);
await session("update-0.2.0.bin", true);
td.kill();
xvfb.kill();
server.kill();
console.log(failures ? `${failures} fallo(s)` : "E2E ACTUALIZADOR OK");
process.exit(failures ? 1 : 0);
