// Tauri v2 では release ビルドで Windows コンソールを抑制する慣例。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    conduction_app::run();
}
