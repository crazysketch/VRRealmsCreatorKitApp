// No console window next to the app in a release build.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    vrrealms_creator_kit_lib::run()
}
