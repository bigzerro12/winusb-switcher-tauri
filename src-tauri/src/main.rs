#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(debug_assertions)]
    {
        let _ = ctrlc::set_handler(|| std::process::exit(0));
    }
    winusb_switcher_lib::run();
}