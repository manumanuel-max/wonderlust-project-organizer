# Wonderlust Project Organizer — Arquitectura y plan técnico

## 1. Decisión de arquitectura

Se mantiene la arquitectura propuesta: **Tauri 2 + React + TypeScript + Tailwind CSS** con
toda la lógica de archivos en **Rust**. No se identificó una alternativa mejor:

| Alternativa | Motivo para descartarla |
|---|---|
| Electron | Instaladores de 150 MB+, más memoria, la lógica de archivos seguiría en Node (menos control sobre E/S, fsync y renombrado atómico). |
| Qt / C++ | Ciclo de desarrollo más lento en UI y sin ventaja real en E/S frente a Rust. |
| Python + PyInstaller | Rendimiento y empaquetado frágil en macOS (firma/notarización), peor manejo de concurrencia. |

El único ajuste: **la lógica de negocio vive en un crate Rust independiente (`wpo-core`)
sin dependencia de Tauri**. Esto permite probarla con `cargo test` sin interfaz ni WebView, y
deja la capa Tauri como una fachada delgada de comandos y eventos.

```
┌──────────────────────────── Ventana (WebView) ────────────────────────────┐
│ React + TS + Tailwind                                                       │
│  Inicio · Crear estructura · Organizar (asistente 5 pasos) · Historial ·   │
│  Configuración                                                              │
│        │ invoke(comando)                 ▲ eventos (progreso)               │
└────────┼─────────────────────────────────┼─────────────────────────────────┘
         ▼                                 │
┌──────────────── src-tauri (fachada) ─────┴────────────────────────────────┐
│ commands.rs  · estado de sesión (inventario, plan, overrides)             │
│ jobs         · hilos de copia, pausa/cancelación, reanudación             │
└────────┬──────────────────────────────────────────────────────────────────┘
         ▼
┌──────────────── wpo-core (Rust puro, 100 % probado) ──────────────────────┐
│ classify   · reglas por extensión (editable exclusivo AEP/AEPX/PRPROJ)    │
│ naming     · nomenclatura DD.MM.AA CLIENTE - PROYECTO y saneado           │
│ structure  · creación de las 5 carpetas                                   │
│ scan       · inventario recursivo sin seguir enlaces simbólicos           │
│ sequence   · detección de secuencias, huecos, casos ambiguos              │
│ folders    · detección funcional de subcarpetas (tarjetas, proxies, ...)  │
│ exclusions · autosaves, cachés, ocultos, sistema, personalizadas          │
│ plan       · plan de destino + overrides del usuario + árbol              │
│ hashing    · SHA-256 por streaming, pre-hash parcial                      │
│ duplicates · duplicados internos y conflictos con el destino              │
│ preflight  · espacio, permisos, recursión, rutas largas, caracteres       │
│ copy       · motor de copia: temporal → verificación → renombrado         │
│              sin sobrescritura, diario (journal) para reanudar            │
│ report     · CSV / JSON / TXT / mapa de rutas                             │
│ settings / history · persistencia JSON local                              │
└───────────────────────────────────────────────────────────────────────────┘
```

## 2. Estructura del repositorio

```
wonderlust-project-organizer/
├── package.json · vite.config.ts · tailwind.config.js · tsconfig.json
├── index.html
├── src/                        # Interfaz React
│   ├── App.tsx  main.tsx  index.css
│   ├── lib/                    # api.ts (comandos tipados), types.ts, format.ts, store.ts
│   ├── components/             # UI reutilizable (Button, Card, Tree, DropZone, ...)
│   └── pages/                  # Home, CreateStructure, Organize/*, History, Settings
├── src-tauri/
│   ├── Cargo.toml              # workspace: app + core
│   ├── tauri.conf.json · capabilities/ · icons/
│   ├── src/                    # main.rs, lib.rs, commands.rs, jobs.rs, state.rs
│   └── core/                   # crate wpo-core
│       ├── src/*.rs
│       └── tests/*.rs          # pruebas de integración en directorios temporales
├── docs/                       # Este documento, manual de usuario
└── .github/workflows/build.yml # Compilación .exe/.msi (Windows) y .app/.dmg (macOS)
```

## 3. Dependencias principales

**Rust (core):** `serde`/`serde_json` (modelo y persistencia), `sha2` (SHA-256),
`unicode-normalization` (comparar nombres NFC/NFD entre macOS y Windows), `fs4` (espacio
libre), `filetime` (conservar fecha de modificación en la copia), `chrono`, `thiserror`,
`uuid`, `crossbeam-channel`, `libc` / `windows-sys` (renombrado sin sobrescritura atómico),
`tempfile` (solo pruebas).

**Rust (app):** `tauri 2`, `tauri-plugin-dialog` (selector de archivos/carpetas),
`tauri-plugin-opener` (abrir en Finder/Explorador).

**Frontend:** `react 18`, `typescript`, `vite`, `tailwindcss 3`, `lucide-react` (iconos),
`zustand` (estado), `@tanstack/react-virtual` (listas de cientos de miles de archivos),
`@tauri-apps/api`, `@tauri-apps/plugin-dialog`.

## 4. Flujo de “Organizar proyecto”

1. **Fuentes** – arrastrar/seleccionar archivos y carpetas, datos del proyecto, destino, modo
   (*Organizar en estructura estándar* o *Backup conservando jerarquía*).
2. **Análisis** – escaneo recursivo → clasificación → secuencias → subcarpetas funcionales →
   renders → exclusiones. El plan vive en Rust; la UI lo consulta paginado (escala a 100 000+
   archivos).
3. **Revisión** – árbol navegable del destino, tabla filtrable, cambios manuales de categoría,
   render final, inclusión/exclusión, decisiones sobre secuencias ambiguas.
4. **Verificación** – permisos, espacio, recursión, rutas, caracteres, duplicados por contenido
   (tamaño → hash parcial → SHA-256) y conflictos con archivos existentes.
5. **Copia** – solo tras confirmación explícita; progreso, pausa, cancelación, reintentos.
6. **Reporte** – validación de integridad, exportación y reintento de fallidos.

## 5. Garantías de seguridad del motor de copia

* Los orígenes se abren **solo en modo lectura**; ningún código escribe, mueve ni borra en
  rutas de origen.
* Cada archivo se escribe a `.<nombre>.wlpart` en el destino, se hace `fsync`, se **relee y
  se compara SHA-256** con el hash calculado durante la lectura del origen, se copia la fecha
  de modificación y se renombra **sin sobrescritura** (`renameat2(RENAME_NOREPLACE)` en Linux,
  `renamex_np(RENAME_EXCL)` en macOS, `MoveFileExW` sin `REPLACE_EXISTING` en Windows).
* Si el destino final ya existe, la operación falla; nunca se sobrescribe.
* Se comprueba que el origen no cambió (tamaño/fecha) durante la copia.
* Cada resultado se registra en un diario JSONL; tras una interrupción se reanuda sin repetir
  archivos ya verificados.
* El reporte solo declara éxito si **todos** los archivos programados están copiados y
  verificados.

## 6. Plan de desarrollo por fases

| Fase | Contenido | Estado en esta entrega |
|---|---|---|
| 1 Base | Arquitectura, UI, navegación, crear estructura, configuración local | Implementada y probada |
| 2 Organización local | Escaneo, clasificación, vista previa, secuencias, copia segura | Implementada y probada |
| 3 Avanzado | Duplicados, exclusiones, renders, validación, historial, reportes, reanudación | Implementada y probada |
| 4 Google Drive | OAuth, selección de carpetas, carga reanudable | **Pendiente** (ver §8) |
| 5 Distribución | Pruebas, empaquetado, instaladores, documentación | Workflow CI + build Linux local |

## 7. Riesgos técnicos y mitigaciones

| Riesgo | Mitigación |
|---|---|
| Vínculos rotos en AEP/PRPROJ tras reorganizar | Advertencia explícita, modo *conservar jerarquía*, mapa de rutas, recomendación de *Collect Files* / *Project Manager*. No se promete revinculación. |
| Falsos positivos en secuencias (fotos de cámara IMG_0001…) | Umbral mínimo, prefijos de cámara y huecos grandes marcan el caso como **ambiguo** y se pide confirmación. |
| Sistemas de archivos sin enlaces duros / sin renombrado atómico (exFAT, SMB) | Cadena de fallback: syscall sin sobrescritura → comprobación + renombrado. Se nunca usa `rename` con reemplazo. |
| Rutas > 260 caracteres en Windows | Uso de prefijo `\\?\` internamente; advertencia en la verificación previa. |
| Nombres válidos en macOS pero no en Windows (`:`, `?`, nombres reservados) | Detección en la verificación previa; opción de ajustar únicamente el nombre de la copia. |
| Volúmenes insensibles a mayúsculas | Colisiones detectadas con clave NFC + minúsculas. |
| La relectura puede venir de caché del SO | Se documenta; la verificación compara bytes escritos vs. leídos, y `fsync` antes de releer. |
| Firma de código (Gatekeeper / SmartScreen) | El workflow deja preparados los secretos de firma; sin certificado, los usuarios verán avisos al instalar. |
| No es posible compilar .exe/.dmg nativos en Linux | GitHub Actions con runners `windows-latest` y `macos-latest`. |

## 8. Google Drive (Fase 4)

Requiere un **ID de cliente OAuth** registrado por Wonderlust en Google Cloud Console (pantalla
de consentimiento y verificación de Google). Sin esas credenciales no es posible implementar ni
probar de forma real la autenticación, así que no se entrega una versión simulada.
Mientras tanto, **Google Drive para escritorio** monta Drive como unidad local
(`G:\` en Windows, `~/Library/CloudStorage/GoogleDrive-*` en macOS); la aplicación detecta esas
ubicaciones y las ofrece como destino local, con todas las garantías del motor de copia.
