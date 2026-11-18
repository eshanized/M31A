// Prevents additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(error) = m31a_desktop_lib::run() {
        eprintln!("M31A Desktop failed to start: {error}");
        std::process::exit(1);
    }
}
