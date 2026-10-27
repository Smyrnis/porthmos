use anyhow::Result;
use tracing_subscriber::prelude::*;

mod app;
mod cli;
mod clock;
mod input;
mod logging;
mod palette;
mod rate;
mod sessions;
mod terminal;
mod theme;
mod widgets;

use app::App;
use porthmos_core::{Core, Environment, Paths, UpdateMode, config, from_os_path};

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        if porthmos_core::tasks::in_supervised_task() {
            tracing::error!(target: "porthmos::tasks", "{panic_info}");
            return;
        }
        let _ = terminal::restore();
        default_hook(panic_info);
    }));
}

fn init_tracing(log_file: Option<&std::path::Path>) {
    let filter = logging::log_filter(std::env::var("RUST_LOG").ok().as_deref());
    let writer = match log_file {
        Some(path) => logging::open_writer_at(path),
        None => Err(anyhow::anyhow!("HOME environment variable is not set")),
    };
    match writer {
        Ok(file) => {
            tracing_subscriber::registry()
                .with(tracing_subscriber::fmt::layer().with_writer(std::sync::Mutex::new(file)).with_filter(filter))
                .init();
        }
        Err(err) => {
            eprintln!("porthmos: failed to open log file, logging disabled: {err}");
            tracing_subscriber::registry()
                .with(tracing_subscriber::fmt::layer().with_writer(std::io::sink).with_filter(filter))
                .init();
        }
    }
}

struct ExitUpdate {
    paths: Paths,
    mode: UpdateMode,
}

fn start_app(
    env: Environment, paths: Paths,
) -> Result<(App, tokio::sync::mpsc::UnboundedReceiver<porthmos_core::Event>, ExitUpdate)> {
    let home = env.home.as_deref().map(from_os_path);
    paths.migrate_legacy_config();
    let (settings, config_warnings) = config::load(&paths)?;
    let (key_bindings, key_warnings) = input::KeyBindings::from_frontend(&settings.frontend);
    let (theme_settings, theme_warnings) = theme::ThemeSettings::from_frontend(&settings.frontend);
    let panel = settings.panel.clone();
    let update_mode = settings.update.mode;
    let exit_update = ExitUpdate { paths: paths.clone(), mode: update_mode };
    let (core, events) = Core::builder().paths(paths).environment(env).settings(settings).release_updates().start()?;

    let theme = theme::Theme::new(theme::ColorDepth::from_environment(), &theme_settings);
    let mut app = App::new(core, from_os_path(&std::env::current_dir()?), &panel, key_bindings)
        .with_theme(theme)
        .with_home(home)
        .with_update_mode(update_mode);
    for warning in config_warnings {
        app.warn(warning.0);
    }
    for warning in key_warnings {
        app.warn(warning);
    }
    for warning in theme_warnings {
        app.warn(warning);
    }
    Ok((app, events, exit_update))
}

const SHUTDOWN_GRACE: std::time::Duration = std::time::Duration::from_millis(500);

fn main() -> Result<()> {
    match cli::interpret(std::env::args().skip(1)) {
        cli::Invocation::Run => {}
        cli::Invocation::Print(text) => {
            println!("{text}");
            return Ok(());
        }
        cli::Invocation::Update { check_only } => return run_update_command(check_only),
        cli::Invocation::Reject(message) => {
            eprintln!("porthmos: {message}");
            eprintln!("try 'porthmos --help'");
            std::process::exit(2);
        }
    }
    porthmos_core::disable_core_dumps();
    let runtime = tokio::runtime::Runtime::new()?;
    let result = runtime.block_on(run());
    runtime.shutdown_timeout(SHUTDOWN_GRACE);
    if let Some(exit) = result? {
        install_update(exit);
    }
    Ok(())
}

fn install_update(exit: ExitUpdate) {
    match cli::install_message(&porthmos_core::apply_staged_update(&exit.paths, exit.mode)) {
        Some(cli::ExitLine::Out(line)) => println!("{line}"),
        Some(cli::ExitLine::Err(line)) => eprintln!("{line}"),
        None => {}
    }
}

fn run_update_command(check_only: bool) -> Result<()> {
    let paths = match Paths::from_env(&Environment::from_process()) {
        Ok(paths) => paths,
        Err(err) => {
            eprintln!("porthmos: {err:#}");
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
            eprintln!("porthmos: {message}");
            std::process::exit(1);
        }
    }
}

async fn run() -> Result<Option<ExitUpdate>> {
    let env = Environment::from_process();
    let paths = Paths::from_env(&env);
    init_tracing(paths.as_ref().ok().map(Paths::log_file).as_deref());

    install_panic_hook();

    let mut terminal = terminal::init()?;

    let result = match paths.and_then(|paths| start_app(env, paths)) {
        Ok((mut app, events, exit)) => app.run(&mut terminal, events).await.map(|()| Some(exit)),
        Err(err) => Err(err),
    };

    terminal::restore()?;

    result
}
