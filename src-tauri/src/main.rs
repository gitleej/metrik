#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let command = std::env::args_os().nth(1);
    if command.as_deref() == Some(std::ffi::OsStr::new("--statusline")) {
        metrik_lib::run_statusline();
        return;
    }
    if command.as_deref() == Some(std::ffi::OsStr::new("--publish-widget-snapshot")) {
        let Some(database_path) = std::env::args_os().nth(2).map(std::path::PathBuf::from) else {
            eprintln!("--publish-widget-snapshot requires a database path");
            std::process::exit(2);
        };
        match metrik_lib::publish_widget_snapshot_from_database(&database_path) {
            Ok(path) => println!("{}", path.display()),
            Err(error) => {
                eprintln!("could not publish WidgetKit snapshot: {error:#}");
                std::process::exit(1);
            }
        }
        return;
    }

    metrik_lib::run();
}
