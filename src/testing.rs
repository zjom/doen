//! Helpers for the unit tests: an app on a temporary store at a fixed time.

use std::path::Path;

use crate::app::App;
use crate::cli::args::CreateCmd;
use crate::commands::create::{self, Draft};
use crate::config::Config;
use crate::dates::Clock;
use crate::item::Kind;

pub const NOW: &str = "2026-10-16T12:00:00+11:00";

pub fn app(root: &Path) -> App {
    app_with(root, Config::default())
}

pub fn app_with(root: &Path, cfg: Config) -> App {
    App {
        root: root.into(),
        cfg,
        clock: Clock::at(
            chrono::DateTime::parse_from_rfc3339(NOW).unwrap(),
            chrono::Weekday::Mon,
        ),
        yes: false,
        color: false,
    }
}

/// Creates an item without a template and returns its ID.
pub fn create(app: &App, kind: Kind, title: &str) -> String {
    let c = CreateCmd {
        title: vec![title.to_string()],
        template: Some("none".into()),
        ..CreateCmd::default()
    };
    let draft = Draft::new(app, kind, c).unwrap();
    let mut tx = app.begin().unwrap();
    let id = create::create(app, &mut tx, &draft).unwrap();
    tx.close(app);
    id
}
