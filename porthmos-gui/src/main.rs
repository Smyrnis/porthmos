#![cfg_attr(windows, windows_subsystem = "windows")]

use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use iced::{Size, window};
use porthmos_core::{Environment, Paths, from_os_path};

mod app;
mod bookmarks;
mod breadcrumbs;
mod cli;
mod config_file;
mod conflicts;
mod connections;
mod console;
mod dialogs;
mod drag;
mod editing;
mod entries;
mod geometry;
mod history_view;
mod icons;
mod menus;
mod modal;
mod names;
mod native;
mod navigation;
mod palette;
mod pane;
mod panel;
mod preview_view;
mod processes;
mod runtime;
mod search;
mod sessions;
mod settings_view;
mod shortcuts;
mod staging;
mod sync_view;
mod terminal;
#[cfg(test)]
mod testing;
mod theme;
mod tools;
mod transfers;
mod update_view;
mod view;

fn main() -> Result<()> {
    match cli::interpret(std::env::args().skip(1)) {
        cli::Invocation::Run => run_window(),
        cli::Invocation::Print(text) => {
            console::attach_parent();
            println!("{text}");
            Ok(())
        }
        cli::Invocation::Update { check_only } => {
            console::attach_parent();
            run_update_command(check_only)
        }
        cli::Invocation::Reject(message) => {
            console::attach_parent();
            eprintln!("porthmos-gui: {message}");
            eprintln!("try 'porthmos-gui --help'");
            std::process::exit(2);
        }
    }
}

const RUNTIME_GRACE: Duration = Duration::from_millis(500);

fn run_window() -> Result<()> {
    porthmos_core::disable_core_dumps();
    staging::remove_stale(&std::env::temp_dir(), staging::STALE_AFTER);
    let runtime = tokio::runtime::Runtime::new()?;
    let guard = runtime.enter();
    let env = Environment::from_process();
    let home = env.home.clone();
    let paths = Paths::from_env(&env)?;
    porthmos_core::logging::init("porthmos-gui", Some(&paths.log_file()));
    let state_dir = paths.state_dir.clone();
    let started = runtime::start(env, paths)?;
    let palette = theme::Palette::new(&started.theme);
    let local_start = match home.or_else(|| std::env::current_dir().ok()) {
        Some(folder) => from_os_path(&folder),
        None => porthmos_core::VfsPath::root(),
    };
    let mut app = app::App::new(started.core, started.events, local_start, palette, started.shortcuts)
        .with_update_mode(started.settings.update_mode)
        .with_settings(started.settings, started.config_file);
    for warning in started.warnings {
        app.warn(warning);
    }
    app.set_state_dir(state_dir);
    let size = Size::new(1100.0, 700.0);
    app.set_window_size(size);
    let slot = Mutex::new(Some(app));
    let settings = window::Settings {
        size,
        min_size: Some(Size::new(480.0, 320.0)),
        exit_on_close_request: false,
        ..window::Settings::default()
    };
    let result = iced::application(
        move || {
            let app = slot.lock().unwrap().take().expect("the application boots once");
            let attach = app.native_task();
            (app, attach)
        },
        app::App::update,
        app::App::view,
    )
    .subscription(app::App::subscription)
    .theme(|app: &app::App| app.palette().iced_theme())
    .title("Porthmos")
    .window(settings)
    .run();
    drop(guard);
    runtime.shutdown_timeout(RUNTIME_GRACE);
    Ok(result?)
}

fn run_update_command(check_only: bool) -> Result<()> {
    let paths = match Paths::from_env(&Environment::from_process()) {
        Ok(paths) => paths,
        Err(err) => {
            eprintln!("porthmos-gui: {err:#}");
            std::process::exit(1);
        }
    };
    let runtime = tokio::runtime::Runtime::new()?;
    match runtime.block_on(porthmos_core::run_update(&paths, check_only)) {
        Ok(update) => {
            println!("{}", cli::describe_update(&update));
            Ok(())
        }
        Err(message) => {
            eprintln!("porthmos-gui: {message}");
            std::process::exit(1);
        }
    }
}
