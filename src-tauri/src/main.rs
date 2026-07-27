#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--statusline")) {
        metrik_lib::run_statusline();
        return;
    }

    metrik_lib::run();
}
