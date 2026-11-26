# Manual de uso — Wonderlust Project Organizer

## 1. Crear la estructura de un proyecto nuevo

1. **Inicio → Crear estructura.**
2. Revisa la **fecha** (hoy por defecto), escribe **cliente** y **proyecto**.
3. Elige la **ubicación**: botón *Examinar* o uno de los accesos rápidos (discos externos, Google
   Drive para escritorio, destinos recientes y rutas frecuentes).
4. Comprueba la vista previa: `25.11.26 BANCO POPULAR - CAMPAÑA NAVIDAD/` con las cinco carpetas.
5. **Crear estructura** → **Abrir en Finder / el Explorador**.

Si ya existe una carpeta con ese nombre, la aplicación no la toca: puedes **completar las
carpetas faltantes** sin modificar nada de lo que hay dentro.

## 2. Organizar un proyecto existente

El asistente tiene cinco pasos. Nada se copia hasta que confirmas en el paso 3.

### Paso 1 — Selección
* Arrastra carpetas o archivos a la zona punteada, o usa *Añadir archivos* / *Añadir carpetas*.
  Si seleccionas **una carpeta**, se trata como el proyecto completo; si seleccionas **varias**,
  cada una conserva su nombre.
* Si la carpeta ya se llama `DD.MM.AA CLIENTE - PROYECTO`, fecha, cliente y proyecto se rellenan solos.
* Elige el **modo**:
  * **Organizar en estructura estándar**: clasifica en las cinco carpetas.
  * **Backup conservando jerarquía**: copia la estructura original tal cual (recomendado cuando
    necesitas abrir los proyectos de After Effects / Premiere sin revincular).
* Elige el **destino**: disco local, SSD/HDD, memoria USB, unidad de red o Google Drive para
  escritorio. Se mostrará el espacio libre.

### Paso 2 — Vista previa
* **Resumen**: total, a copiar, excluidos, secuencias, renders, proyectos de Adobe, archivos sin
  clasificación específica y carpetas funcionales; desglose por categoría.
* **Estructura de destino**: árbol navegable; al elegir una carpeta, la tabla muestra su contenido.
* **Archivos**: búsqueda y filtros (categoría, excluidos, renders, secuencias, Adobe, cambios
  manuales). Selecciona filas para:
  * cambiar la categoría (los `.aep`, `.aepx` y `.prproj` siempre van a `4. editable`, y nada más
    puede ir allí);
  * **marcar / quitar render final**;
  * mover a una **subcarpeta** concreta;
  * **excluir / incluir**; **restablecer**.
* **Secuencias**: revisa fotogramas faltantes. Los grupos **ambiguos** (fotos de cámara
  numeradas, grupos de 2–4 imágenes, numeración con muchos huecos) deben confirmarse como
  *secuencia* o *imágenes independientes* antes de continuar.
* **Carpetas detectadas**: tarjetas de cámara, proxies, renders, música, SFX, locuciones, stock,
  gráficos… Desactiva un grupo si prefieres que sus archivos se clasifiquen uno a uno.
* **Exclusiones**: activa o desactiva autosaves, cachés, proxies, temporales, ocultos, etc. Se ve
  la cantidad y el peso excluidos.
* **Advertencias**: en especial, la de **proyectos de Adobe**.

### Paso 3 — Verificación
La aplicación comprueba permisos, espacio, rutas largas, caracteres incompatibles con Windows,
FAT32, destino dentro del origen, archivos ilegibles, duplicados por contenido y conflictos con
archivos que ya existen en el destino.

* **Bloqueante (rojo)**: hay que corregirlo para poder copiar (p. ej. espacio insuficiente).
* **Conflictos de nombre**: elige *Omitir*, *Conservar ambos* (la copia nueva recibe `_copiaN`) o
  *Renombrar la copia…*. Con *Aplicar a conflictos equivalentes* la decisión se extiende a los del
  mismo tipo. **Nunca se sobrescribe un archivo existente.**
* **Continuar** abre la ventana **¿Quieres copiar o mover los archivos?**
  * **Copiar (recomendado):** los originales quedan intactos. Necesita espacio libre para todo.
  * **Mover:** los archivos salen de su ubicación original. En el **mismo disco** es instantáneo y
    no ocupa espacio extra. Hacia **otro disco**, cada archivo se copia, se verifica con SHA-256 y
    solo entonces el original va a la **Papelera** (o se elimina, si lo eliges). Los archivos
    excluidos u omitidos se quedan en el origen, y se eliminan las carpetas que queden vacías
    (no la carpeta que seleccionaste). Hay que marcar *"Entiendo que los archivos dejarán de estar
    en su ubicación original"*.
  * Si no hay espacio para copiar pero sí para mover (mismo disco), solo se ofrece mover.
* La ventana muestra un resumen final. Si hay proyectos de Adobe en modo organizar, debes
  confirmar que entiendes que se trata de un **backup organizado de archivos**, no de un
  proyecto recopilado listo para abrir.

### Paso 4 — Copia
Barra general, archivo en curso, velocidad, tiempo transcurrido y restante, archivos procesados y
errores. **Pausar / Reanudar / Cancelar** en cualquier momento. Puedes navegar por la aplicación
mientras copia.

### Paso 5 — Reporte
* **Backup completado y verificado** solo aparece si todos los archivos programados se copiaron y
  verificaron. En cualquier otro caso se indica claramente que el backup **no** está completo.
* Exporta **TXT**, **CSV** (separado por `;`, se abre bien en Excel en español), **JSON** o el
  **mapa de rutas** (original → nueva).
* **Reintentar fallidos** repite solo lo que falló o quedó pendiente.

## 3. Si se cierra la aplicación o se desconecta el disco

Al volver a abrir, **Inicio → Operaciones interrumpidas → Reanudar**. Los archivos ya copiados y
verificados no se repiten; los temporales a medio escribir (`.wlpart`) se descartan. También
puedes reanudar desde **Historial**.

## 4. After Effects y Premiere Pro

* Los proyectos guardan rutas a sus recursos. Reorganizar una copia puede romper vínculos.
* Para un proyecto que se pueda abrir directamente en otro equipo, usa antes **File → Dependencies
  → Collect Files** (After Effects) o **File → Project Manager** (Premiere Pro), o el modo
  **Backup conservando jerarquía**.
* El **mapa de rutas** del reporte indica dónde quedó cada archivo para revincular manualmente.
* La aplicación **nunca** modifica el contenido de archivos `.aep`, `.aepx` o `.prproj`.
* **Mover** un proyecto con archivos de Adobe rompe también los vínculos del proyecto original.
  Si necesitas abrirlo sin revincular, elige **Copiar**.

## 5. Recomendaciones

* Discos mecánicos (HDD): *Configuración → Rendimiento → Copias simultáneas = 1*. SSD/NVMe: 2–4.
* Memorias USB en FAT32 no admiten archivos de 4 GB o más: formatea en exFAT.
* Mantén la verificación **SHA-256 completa** para backups de entrega.
