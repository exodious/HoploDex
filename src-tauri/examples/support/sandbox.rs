//! Where the human-testing seed may write (constitution 1.2.0, research.md
//! §21). The real databases may be anywhere the user chose, so the seed
//! cannot refuse one known directory. It allows only a directory it made
//! itself: one that does not exist yet (or is empty), or one holding the
//! marker file it left there. It also refuses any target that is, or lies
//! inside, the real data, config or documents directory, resolved from the
//! environment it was given.
//!
//! Shared by `examples/human_seed.rs` and `tests/seed_sandbox_test.rs`
//! through `#[path]`, so it is tested without shipping in the application.

use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// The file that marks a directory as the seed's own.
pub const MARKER: &str = ".hoplodex-sandbox";

/// Accepts `target` as a sandbox, creating it and writing [`MARKER`] when
/// it is new or empty, or explains why not. `env` reads an environment
/// variable (`std::env::var_os` in the seed; a fake in the tests).
pub fn check_sandbox(target: &Path, env: &dyn Fn(&str) -> Option<OsString>) -> Result<(), String> {
    let resolved = resolve(target);
    for (what, real) in real_directories(env) {
        if resolved.starts_with(resolve(&real)) {
            return Err(format!(
                "refusing to seed {}: it is inside the real {what} directory {}",
                target.display(),
                real.display()
            ));
        }
    }

    if target.join(MARKER).is_file() {
        return Ok(());
    }
    match fs::read_dir(target) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                return Err(format!(
                    "refusing to seed {}: it is not empty and was not made by the seed \
                     (no {MARKER} file). Choose a new directory, or delete this one first.",
                    target.display()
                ));
            }
        }
        Err(_) if target.exists() => {
            return Err(format!("refusing to seed {}: it is not a directory", target.display()));
        }
        Err(_) => {}
    }
    fs::create_dir_all(target)
        .and_then(|()| fs::write(target.join(MARKER), b"Made by examples/human_seed.rs\n"))
        .map_err(|err| format!("could not prepare {}: {err}", target.display()))
}

/// The directories real application data lives in: the app data and config
/// directories and the documents folder a new database is suggested in.
fn real_directories(env: &dyn Fn(&str) -> Option<OsString>) -> Vec<(&'static str, PathBuf)> {
    let home = env("HOME").map(PathBuf::from);
    let from = |name: &str, fallback: &str| {
        env(name).map(PathBuf::from).or_else(|| home.as_ref().map(|home| home.join(fallback)))
    };
    let data = from("XDG_DATA_HOME", ".local/share");
    let config = from("XDG_CONFIG_HOME", ".config");
    let documents = env("XDG_DOCUMENTS_DIR")
        .map(PathBuf::from)
        .or_else(|| user_dirs_documents(config.as_deref()?, home.as_deref()?))
        .or_else(|| home.as_ref().map(|home| home.join("Documents")));
    [("data", data), ("config", config), ("documents", documents)]
        .into_iter()
        .filter_map(|(what, dir)| Some((what, dir?)))
        .collect()
}

/// `XDG_DOCUMENTS_DIR` from `<config>/user-dirs.dirs`, as
/// `xdg-user-dirs-update` writes it: `XDG_DOCUMENTS_DIR="$HOME/Documents"`.
fn user_dirs_documents(config: &Path, home: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(config.join("user-dirs.dirs")).ok()?;
    let value = text
        .lines()
        .rev()
        .find_map(|line| line.trim().strip_prefix("XDG_DOCUMENTS_DIR="))?
        .trim()
        .trim_matches('"');
    Some(match value.strip_prefix("$HOME") {
        Some(rest) => home.join(rest.trim_start_matches('/')),
        None => PathBuf::from(value),
    })
}

/// `path` made absolute with symlinks and `..` resolved as far as it
/// exists, and the rest (which does not exist yet) normalized lexically.
fn resolve(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    let mut existing = absolute.as_path();
    let mut rest: Vec<OsString> = Vec::new();
    loop {
        if let Ok(real) = existing.canonicalize() {
            let mut resolved = real;
            for component in rest.iter().rev() {
                push_normalized(&mut resolved, component);
            }
            return resolved;
        }
        match (existing.parent(), existing.components().next_back()) {
            (Some(parent), Some(last)) => {
                rest.push(last.as_os_str().to_owned());
                existing = parent;
            }
            _ => {
                // Nothing of it exists: normalize the whole path lexically.
                let mut resolved = PathBuf::new();
                for component in absolute.components() {
                    push_normalized(&mut resolved, component.as_os_str());
                }
                return resolved;
            }
        }
    }
}

fn push_normalized(path: &mut PathBuf, component: &std::ffi::OsStr) {
    match Path::new(component).components().next() {
        Some(Component::ParentDir) => {
            path.pop();
        }
        Some(Component::CurDir) | None => {}
        Some(_) => path.push(component),
    }
}
