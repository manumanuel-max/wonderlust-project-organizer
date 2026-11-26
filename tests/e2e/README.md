# Pruebas de extremo a extremo

Manejan la **aplicación compilada real** (interfaz + backend Rust) con WebDriver, sobre un
proyecto de prueba generado en `/tmp/claude-0/e2e`. Nunca tocan archivos reales.

Requisitos (Linux): `xvfb`, `webkit2gtk-driver` y `cargo install tauri-driver`.

```bash
npm install
```

```bash
npm run tauri build -- --debug --no-bundle
```

```bash
cd tests/e2e && npm install && npm run flujo -- ../../src-tauri/target/debug/wonderlust-project-organizer
```

```bash
cd tests/e2e && npm run reanudacion -- ../../src-tauri/target/debug/wonderlust-project-organizer
```

* `e2e.mjs`: crear estructura, organizar (soltar carpeta, vista previa, secuencias ambiguas,
  conflicto con archivo existente → conservar ambos, confirmación Adobe, pausa/reanudación,
  reporte) y comprobación del resultado en disco, incluida la integridad de los originales.
* `e2e-resume.mjs`: cierra la aplicación con `SIGKILL` a mitad de la copia, la vuelve a abrir,
  reanuda desde Inicio y comprueba que no se repiten archivos ya verificados.

Atención: ambos scripts borran y recrean la configuración de la aplicación del usuario actual
(`~/.config/com.wonderlust.projectorganizer`, `~/.local/share/com.wonderlust.projectorganizer`).
Ejecútalos en una máquina o usuario de pruebas.

El evento de soltar archivos del sistema operativo se emite como el evento interno de Tauri
`tauri://drag-drop`, porque Xvfb no puede simular un arrastre desde un gestor de archivos; el
resto del flujo usa la interfaz y el backend reales.
