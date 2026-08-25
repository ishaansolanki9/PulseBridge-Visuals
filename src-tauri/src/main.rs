#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    pulsebridge_lib::run();
}
