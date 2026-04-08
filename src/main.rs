use gtk::{glib, prelude::*};
use std::rc::Rc;

mod artwork;
mod cli;
mod config;
mod metadata;
mod model;
mod monitor;
mod motion;
mod mpris;
mod player;
mod timestamp;
mod transitions;
mod ui;

use crate::cli::{StartupAction, USAGE};

fn main() -> glib::ExitCode {
    let action = match StartupAction::from_env() {
        Ok(action) => action,
        Err(message) => {
            eprintln!("{message}");
            eprintln!("{USAGE}");
            return glib::ExitCode::FAILURE;
        }
    };

    if matches!(&action, StartupAction::Help) {
        println!("{USAGE}");
        return glib::ExitCode::SUCCESS;
    }

    if matches!(&action, StartupAction::InitConfig) {
        return match config::init_config_file() {
            Ok(path) => {
                println!("covermint: wrote config template to {}", path.display());
                glib::ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("covermint: failed to initialize config: {error}");
                glib::ExitCode::FAILURE
            }
        };
    }

    let app = gtk::Application::builder()
        .application_id("dev.tgz.covermint")
        .build();

    app.connect_activate(move |app| match &action {
        StartupAction::Help | StartupAction::InitConfig => app.quit(),
        StartupAction::ListMonitors => {
            monitor::list_monitors();
            app.quit();
        }
        StartupAction::ListPlayers => {
            player::list_players();
            app.quit();
        }
        StartupAction::Run(config) => {
            if !gtk4_layer_shell::is_supported() {
                eprintln!("gtk4-layer-shell is not supported by this compositor/session");
                app.quit();
                return;
            }

            ui::build_ui(app, Rc::new((**config).clone()));
        }
    });

    app.run_with_args(&["covermint"])
}
