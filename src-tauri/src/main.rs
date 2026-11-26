// Evita una consola adicional en Windows en modo release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    wonderlust_project_organizer_lib::run()
}
