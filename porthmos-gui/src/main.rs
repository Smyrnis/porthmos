#![cfg_attr(windows, windows_subsystem = "windows")]

use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use porthmos_core::{Environment, Paths};

mod app;
mod bookmarks;
mod breadcrumbs;
mod cli;
mod config_file;
mod conflicts;
mod connections;
mod console;
mod dialogs;
mod display;
mod drag;
mod editing;
mod entries;
mod geometry;
mod history_view;
mod icons;
mod main_window;
mod menus;
mod modal;
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
mod window_memory;
mod windows;

fn main() -> Result<()> {
    if let Some(code) = porthmos_ssh::askpass::run_if_requested() {
        std::process::exit(code);
    }
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
    if display::is_missing() {
        eprintln!("{}", display::NO_DISPLAY_MESSAGE);
        std::process::exit(1);
    }
    porthmos_core::disable_core_dumps();
    staging::remove_stale(&std::env::temp_dir(), staging::STALE_AFTER);
    let runtime = tokio::runtime::Runtime::new()?;
    let guard = runtime.enter();
    let env = Environment::from_process();
    let home = env.home.clone();
    let paths = Paths::from_env(&env)?;
    porthmos_core::logging::init("porthmos-gui", Some(&paths.log_file()));
    let state_dir = paths.state_dir.clone();
    let exit_paths = paths.clone();
    let started = runtime::start(env, paths)?;
    let update_mode = started.settings.update_mode;
    let palette = theme::Palette::new(&started.theme);
    let local_start = runtime::local_start(started.core.remembered_local_folder(), home, std::env::current_dir().ok());
    let mut app = app::App::new(started.core, started.events, local_start, palette, started.shortcuts)
        .with_update_mode(update_mode)
        .with_settings(started.settings, started.config_file);
    for warning in started.warnings {
        app.warn(warning);
    }
    let memory_file = window_memory::file_in(&state_dir);
    let remembered = window_memory::WindowMemory::load(&memory_file);
    if let Some(warning) = remembered.warning {
        app.warn(warning);
    }
    app.set_state_dir(state_dir);
    let main_settings = remembered.memory.applied_to(windows::WindowKind::Main, main_window::settings());
    app.set_window_size(main_settings.size);
    let (main_id, open_main) = iced::window::open(main_settings);
    let mut app = app.with_main_window(main_id);
    app.set_window_memory(remembered.memory, memory_file);
    let slot = Mutex::new(Some((app, open_main)));
    let result = iced::daemon(
        move || {
            let (app, open_main) = slot.lock().unwrap().take().expect("the application boots once");
            (app, open_main.map(|_| app::Message::MainWindowOpened))
        },
        app::App::update,
        app::App::view_of,
    )
    .subscription(app::App::subscription)
    .theme(|app: &app::App, _window| app.palette().iced_theme())
    .title(app::App::title_of)
    .run();
    drop(guard);
    runtime.shutdown_timeout(RUNTIME_GRACE);
    install_update(&exit_paths, runtime::update_mode_on_exit(&exit_paths, update_mode));
    Ok(result?)
}

fn install_update(paths: &Paths, mode: porthmos_core::UpdateMode) {
    let identity = cli::app_identity();
    match porthmos_core::apply_staged_update(paths, mode, &identity).exit_line(identity.product.program()) {
        Some(porthmos_core::ExitLine::Out(line)) => println!("{line}"),
        Some(porthmos_core::ExitLine::Err(line)) => eprintln!("{line}"),
        None => {}
    }
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
    match runtime.block_on(porthmos_core::run_update(&paths, check_only, &cli::app_identity())) {
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
