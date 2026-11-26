//! Integración con el sistema: "Organizar con Wonderlust" en el menú
//! contextual de carpetas y recepción de rutas al abrir la aplicación.
//!
//! * Windows: clave en `HKCU\Software\Classes\Directory\shell\WonderlustOrganizar`
//!   (también la crea/elimina el instalador NSIS).
//! * macOS: Acción rápida de Finder en `~/Library/Services/Organizar con Wonderlust.workflow`,
//!   que abre la app con `open -b`. Además se puede arrastrar una carpeta al icono del Dock.
//! * Linux: script de Nautilus (Archivos) en `~/.local/share/nautilus/scripts/`.
//!
//! Las carpetas recibidas solo se cargan como origen en "Organizar proyecto";
//! no se inicia ninguna copia sin confirmación del usuario.

use crate::state::AppState;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

pub const MENU_LABEL: &str = "Organizar con Wonderlust";
#[cfg(target_os = "macos")]
pub const BUNDLE_ID: &str = "com.wonderlust.projectorganizer";

/// Rutas recibidas antes de que la interfaz estuviera lista.
#[derive(Default)]
pub struct LaunchPaths(pub Mutex<Vec<String>>);

/// Extrae rutas existentes de los argumentos de la línea de comandos.
pub fn paths_from_args<I: IntoIterator<Item = String>>(args: I) -> Vec<String> {
    args.into_iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .map(|a| {
            a.strip_prefix("file://")
                .map(|p| percent_decode(p))
                .unwrap_or(a)
        })
        .filter(|a| std::path::Path::new(a).exists())
        .collect()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Entrega rutas a la interfaz (o las guarda si aún no cargó) y trae la ventana al frente.
pub fn deliver(app: &AppHandle, paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }
    if let Some(state) = app.try_state::<LaunchPaths>() {
        state.0.lock().unwrap().extend(paths.clone());
    }
    let _ = app.emit("open-paths", paths);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[tauri::command]
pub fn take_launch_paths(paths: State<LaunchPaths>) -> Vec<String> {
    std::mem::take(&mut *paths.0.lock().unwrap())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextMenuStatus {
    pub supported: bool,
    pub installed: bool,
    pub location: String,
    pub hint: String,
}

#[cfg(target_os = "macos")]
fn workflow_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Services").join(format!("{MENU_LABEL}.workflow")))
}

#[cfg(target_os = "linux")]
fn nautilus_script_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(base.join("nautilus/scripts").join(MENU_LABEL))
}

#[cfg(windows)]
const WIN_KEY: &str = r"Software\Classes\Directory\shell\WonderlustOrganizar";

pub fn status() -> ContextMenuStatus {
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        let installed = winreg::RegKey::predef(HKEY_CURRENT_USER).open_subkey(WIN_KEY).is_ok();
        return ContextMenuStatus {
            supported: true,
            installed,
            location: format!(r"HKEY_CURRENT_USER\{WIN_KEY}"),
            hint: "En Windows 11 aparece en «Mostrar más opciones» (o Mayús + clic derecho).".into(),
        };
    }
    #[cfg(target_os = "macos")]
    {
        let p = workflow_path();
        return ContextMenuStatus {
            supported: p.is_some(),
            installed: p.as_ref().map(|p| p.exists()).unwrap_or(false),
            location: p.map(|p| p.display().to_string()).unwrap_or_default(),
            hint: "En Finder: clic derecho sobre una carpeta → Acciones rápidas (o Servicios) → «Organizar con Wonderlust». \
                   Si no aparece, actívala en Ajustes del Sistema → General → Ítems de inicio y extensiones → Finder. \
                   También puedes arrastrar una carpeta al icono de la app en el Dock."
                .into(),
        };
    }
    #[cfg(target_os = "linux")]
    {
        let p = nautilus_script_path();
        return ContextMenuStatus {
            supported: p.is_some(),
            installed: p.as_ref().map(|p| p.exists()).unwrap_or(false),
            location: p.map(|p| p.display().to_string()).unwrap_or_default(),
            hint: "En Archivos (Nautilus): clic derecho → Scripts → «Organizar con Wonderlust».".into(),
        };
    }
    #[allow(unreachable_code)]
    ContextMenuStatus { supported: false, installed: false, location: String::new(), hint: String::new() }
}

/// Instala o retira la opción del menú contextual.
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_CURRENT_USER, KEY_ALL_ACCESS};
        let hkcu = winreg::RegKey::predef(HKEY_CURRENT_USER);
        if enabled {
            let (key, _) = hkcu.create_subkey(WIN_KEY).map_err(|e| e.to_string())?;
            key.set_value("MUIVerb", &MENU_LABEL).map_err(|e| e.to_string())?;
            key.set_value("Icon", &format!("{},0", exe.display())).map_err(|e| e.to_string())?;
            let (cmd, _) = key.create_subkey("command").map_err(|e| e.to_string())?;
            cmd.set_value("", &format!("\"{}\" \"%1\"", exe.display())).map_err(|e| e.to_string())?;
        } else if let Ok(parent) = hkcu.open_subkey_with_flags(r"Software\Classes\Directory\shell", KEY_ALL_ACCESS) {
            let _ = parent.delete_subkey_all("WonderlustOrganizar");
        }
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        let _ = &exe;
        let dir = workflow_path().ok_or("No se encontró la carpeta personal")?;
        if !enabled {
            if dir.exists() {
                std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
            }
            return Ok(());
        }
        let contents = dir.join("Contents");
        std::fs::create_dir_all(&contents).map_err(|e| e.to_string())?;
        std::fs::write(contents.join("Info.plist"), macos_info_plist()).map_err(|e| e.to_string())?;
        std::fs::write(contents.join("document.wflow"), macos_document_wflow()).map_err(|e| e.to_string())?;
        // Pedir a macOS que vuelva a leer los servicios.
        let _ = std::process::Command::new("/System/Library/CoreServices/pbs").arg("-update").status();
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        let p = nautilus_script_path().ok_or("No se encontró la carpeta personal")?;
        if !enabled {
            if p.exists() {
                std::fs::remove_file(&p).map_err(|e| e.to_string())?;
            }
            return Ok(());
        }
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
        // Con AppImage, la ruta estable es la del propio .AppImage.
        let target = std::env::var("APPIMAGE").map(PathBuf::from).unwrap_or(exe);
        let script = format!(
            "#!/bin/sh\n# {MENU_LABEL}: abre las carpetas seleccionadas en Wonderlust Project Organizer.\nexec \"{}\" \"$@\"\n",
            target.display()
        );
        std::fs::write(&p, script).map_err(|e| e.to_string())?;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    {
        let _ = (enabled, exe);
        Err("No disponible en este sistema".into())
    }
}

#[cfg(target_os = "macos")]
fn macos_info_plist() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>NSServices</key>
	<array>
		<dict>
			<key>NSBackgroundColorName</key>
			<string>background</string>
			<key>NSIconName</key>
			<string>NSActionTemplate</string>
			<key>NSMenuItem</key>
			<dict>
				<key>default</key>
				<string>{MENU_LABEL}</string>
			</dict>
			<key>NSMessage</key>
			<string>runWorkflowAsService</string>
			<key>NSRequiredContext</key>
			<dict>
				<key>NSApplicationIdentifier</key>
				<string>com.apple.finder</string>
			</dict>
			<key>NSSendFileTypes</key>
			<array>
				<string>public.folder</string>
			</array>
		</dict>
	</array>
</dict>
</plist>
"#
    )
}

#[cfg(target_os = "macos")]
fn macos_document_wflow() -> String {
    let command = format!("open -b {BUNDLE_ID} \"$@\"");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>AMApplicationBuild</key>
	<string>523</string>
	<key>AMApplicationVersion</key>
	<string>2.10</string>
	<key>AMDocumentVersion</key>
	<string>2</string>
	<key>actions</key>
	<array>
		<dict>
			<key>action</key>
			<dict>
				<key>AMAccepts</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Optional</key>
					<true/>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>AMActionVersion</key>
				<string>2.0.3</string>
				<key>AMApplication</key>
				<array>
					<string>Automator</string>
				</array>
				<key>AMParameterProperties</key>
				<dict>
					<key>COMMAND_STRING</key>
					<dict/>
					<key>CheckedForUserDefaultShell</key>
					<dict/>
					<key>inputMethod</key>
					<dict/>
					<key>shell</key>
					<dict/>
					<key>source</key>
					<dict/>
				</dict>
				<key>AMProvides</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>ActionBundlePath</key>
				<string>/System/Library/Automator/Run Shell Script.action</string>
				<key>ActionName</key>
				<string>Run Shell Script</string>
				<key>ActionParameters</key>
				<dict>
					<key>COMMAND_STRING</key>
					<string>{command}</string>
					<key>CheckedForUserDefaultShell</key>
					<true/>
					<key>inputMethod</key>
					<integer>1</integer>
					<key>shell</key>
					<string>/bin/sh</string>
					<key>source</key>
					<string></string>
				</dict>
				<key>BundleIdentifier</key>
				<string>com.apple.RunShellScript</string>
				<key>CFBundleVersion</key>
				<string>2.0.3</string>
				<key>CanShowSelectedItemsWhenRun</key>
				<false/>
				<key>CanShowWhenRun</key>
				<true/>
				<key>Category</key>
				<array>
					<string>AMCategoryUtilities</string>
				</array>
				<key>Class Name</key>
				<string>RunShellScriptAction</string>
				<key>InputUUID</key>
				<string>6B2E8B1C-1D0C-4F6E-9D2A-3C1F0E5A7B10</string>
				<key>Keywords</key>
				<array>
					<string>Shell</string>
					<string>Script</string>
				</array>
				<key>OutputUUID</key>
				<string>7C3F9C2D-2E1D-4A7F-8E3B-4D2A1F6B8C21</string>
				<key>UUID</key>
				<string>8D4A0D3E-3F2E-4B80-9F4C-5E3B2A7C9D32</string>
				<key>UnlocalizedApplications</key>
				<array>
					<string>Automator</string>
				</array>
				<key>arguments</key>
				<dict/>
				<key>isViewVisible</key>
				<integer>1</integer>
				<key>location</key>
				<string>309.000000:253.000000</string>
				<key>nibPath</key>
				<string>/System/Library/Automator/Run Shell Script.action/Contents/Resources/Base.lproj/main.nib</string>
			</dict>
			<key>isViewVisible</key>
			<integer>1</integer>
		</dict>
	</array>
	<key>connectors</key>
	<dict/>
	<key>workflowMetaData</key>
	<dict>
		<key>applicationBundleIDsByPath</key>
		<dict/>
		<key>applicationPaths</key>
		<array/>
		<key>inputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject.folder</string>
		<key>outputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>presentationMode</key>
		<integer>15</integer>
		<key>processesInput</key>
		<false/>
		<key>serviceApplicationBundleID</key>
		<string>com.apple.finder</string>
		<key>serviceApplicationPath</key>
		<string>/System/Library/CoreServices/Finder.app</string>
		<key>serviceInputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject.folder</string>
		<key>serviceOutputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>serviceProcessesInput</key>
		<false/>
		<key>systemImageName</key>
		<string>NSActionTemplate</string>
		<key>useAutomaticInputType</key>
		<false/>
		<key>workflowTypeIdentifier</key>
		<string>com.apple.Automator.servicesMenu</string>
	</dict>
</dict>
</plist>
"#
    )
}

#[tauri::command]
pub fn context_menu_status() -> ContextMenuStatus {
    status()
}

#[tauri::command]
pub fn set_context_menu(state: State<AppState>, enabled: bool) -> Result<ContextMenuStatus, String> {
    set_enabled(enabled)?;
    let mut s = state.settings.lock().unwrap();
    s.context_menu = enabled;
    let _ = wpo_core::settings::save(&state.settings_path(), &s);
    Ok(status())
}

/// Al iniciar: si la opción está activada, se (re)instala para que apunte a la
/// ubicación actual de la aplicación.
pub fn sync_on_startup(state: &AppState) {
    if state.settings.lock().unwrap().context_menu {
        let _ = set_enabled(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_args_and_file_urls() {
        let d = std::env::temp_dir().join("wpo arg prueba ñ");
        std::fs::create_dir_all(&d).unwrap();
        let url = format!("file://{}", d.display().to_string().replace(' ', "%20").replace('ñ', "%C3%B1"));
        let got = paths_from_args(vec!["app".into(), "--flag".into(), d.display().to_string(), url, "/no/existe".into()]);
        assert_eq!(got, vec![d.display().to_string(), d.display().to_string()]);
        let _ = std::fs::remove_dir(&d);
    }
}
