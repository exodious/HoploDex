//! The human-testing seed writes only into a sandbox it made, and never into
//! the real data, config or documents directory (constitution 1.2.0,
//! research.md §21). Every case builds its own environment around a temp
//! `HOME`; the developer's real directories are never resolved.

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

#[path = "../examples/support/sandbox.rs"]
mod sandbox;

use sandbox::{MARKER, check_sandbox};

/// A fake home and the variables a test sets on top of `HOME`.
struct Env {
    home: TempDir,
    vars: HashMap<&'static str, OsString>,
}

impl Env {
    fn new() -> Self {
        let home = TempDir::new().unwrap();
        let vars = HashMap::from([("HOME", home.path().as_os_str().to_owned())]);
        Self { home, vars }
    }

    fn with(mut self, name: &'static str, value: &Path) -> Self {
        self.vars.insert(name, value.as_os_str().to_owned());
        self
    }

    fn home(&self) -> &Path {
        self.home.path()
    }

    fn check(&self, target: &Path) -> Result<(), String> {
        check_sandbox(target, &|name: &str| self.vars.get(name).cloned())
    }
}

fn scratch(env: &Env, name: &str) -> PathBuf {
    env.home().join("work").join(name)
}

#[test]
fn a_new_directory_is_accepted_and_marked() {
    let env = Env::new();
    let target = scratch(&env, "sandbox");

    env.check(&target).unwrap();

    assert!(target.join(MARKER).is_file());
}

#[test]
fn a_directory_the_seed_marked_is_accepted_again() {
    let env = Env::new();
    let target = scratch(&env, "sandbox");
    env.check(&target).unwrap();
    fs::write(target.join("Main collection.hoplodex"), b"seeded").unwrap();

    env.check(&target).unwrap();
}

#[test]
fn an_empty_existing_directory_is_accepted_and_marked() {
    let env = Env::new();
    let target = scratch(&env, "empty");
    fs::create_dir_all(&target).unwrap();

    env.check(&target).unwrap();

    assert!(target.join(MARKER).is_file());
}

#[test]
fn a_directory_with_content_but_no_marker_is_refused_and_left_alone() {
    let env = Env::new();
    let target = scratch(&env, "someone's files");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("notes.txt"), b"mine").unwrap();

    let refused = env.check(&target).unwrap_err();

    assert!(refused.contains("not empty"), "{refused}");
    assert!(!target.join(MARKER).exists());
}

#[test]
fn a_file_is_refused() {
    let env = Env::new();
    let target = scratch(&env, "a file");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, b"x").unwrap();

    assert!(env.check(&target).is_err());
}

#[test]
fn the_real_data_directory_and_anything_inside_it_are_refused() {
    let env = Env::new();
    let data = env.home().join(".local/share");

    assert!(env.check(&data).is_err());
    assert!(env.check(&data.join("com.hoplodex.app")).is_err());
    assert!(env.check(&data.join("new/place")).is_err());

    let custom = env.home().join("elsewhere/data");
    let env = env.with("XDG_DATA_HOME", &custom);
    assert!(env.check(&custom.join("seed")).is_err());
}

#[test]
fn the_real_config_directory_and_anything_inside_it_are_refused() {
    let env = Env::new();
    assert!(env.check(&env.home().join(".config/io.github.exodious.HoploDex")).is_err());

    let custom = env.home().join("cfg");
    let env = env.with("XDG_CONFIG_HOME", &custom);
    assert!(env.check(&custom).is_err());
    assert!(env.check(&custom.join("x")).is_err());
}

#[test]
fn the_documents_directory_is_refused_by_default_by_variable_and_by_user_dirs() {
    // The fallback.
    let env = Env::new();
    assert!(env.check(&env.home().join("Documents/HoploDex")).is_err());

    // Named by the environment.
    let docs = env.home().join("Papers");
    let env = env.with("XDG_DOCUMENTS_DIR", &docs);
    assert!(env.check(&docs.join("HoploDex")).is_err());

    // Named by user-dirs.dirs in the config directory, relative to $HOME.
    let env = Env::new();
    let config = env.home().join(".config");
    fs::create_dir_all(&config).unwrap();
    fs::write(
        config.join("user-dirs.dirs"),
        "# written by xdg-user-dirs-update\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_DOCUMENTS_DIR=\"$HOME/Dokumente\"\n",
    )
    .unwrap();
    assert!(env.check(&env.home().join("Dokumente/HoploDex")).is_err());
    // …and the default no longer applies once user-dirs.dirs names another.
    env.check(&env.home().join("Documents/sandbox")).unwrap();
}

#[test]
fn a_marked_directory_inside_a_real_directory_is_still_refused() {
    let env = Env::new();
    let target = env.home().join("Documents/old sandbox");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join(MARKER), b"").unwrap();

    assert!(env.check(&target).is_err());
}

#[test]
fn dot_dot_cannot_sneak_into_a_real_directory() {
    let env = Env::new();
    fs::create_dir_all(env.home().join("work")).unwrap();
    let sneaky = env.home().join("work/../.local/share/seed");

    assert!(env.check(&sneaky).is_err());
}

#[cfg(unix)]
#[test]
fn a_symlink_into_a_real_directory_is_refused() {
    let env = Env::new();
    let data = env.home().join(".local/share");
    fs::create_dir_all(&data).unwrap();
    fs::create_dir_all(env.home().join("work")).unwrap();
    let link = env.home().join("work/link");
    std::os::unix::fs::symlink(&data, &link).unwrap();

    assert!(env.check(&link.join("seed")).is_err());
}
