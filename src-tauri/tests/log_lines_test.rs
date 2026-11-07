//! 013 FR-030: a log macro names a path only through `logging::place`, which
//! gives its shape. This reads the source, so it catches the patterns below and
//! not every value a line could interpolate: `docs/attack-surface.md` keeps a
//! review question for the rest.

use std::fs;
use std::path::{Path, PathBuf};

fn rust_files(folder: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

/// The body of each log macro in `text`, with its line number. Parentheses in
/// a string are rare enough here that a miscount would show as a parse of too
/// much text, which only makes the check stricter.
fn macro_bodies(text: &str) -> Vec<(usize, &str)> {
    let mut bodies = Vec::new();
    for level in ["error", "warn", "info", "debug", "trace"] {
        let opening = format!("log::{level}!(");
        for (start, _) in text.match_indices(&opening) {
            let body_start = start + opening.len();
            let mut depth = 1;
            let mut end = body_start;
            for (offset, c) in text[body_start..].char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                if depth == 0 {
                    end = body_start + offset;
                    break;
                }
            }
            bodies.push((text[..start].matches('\n').count() + 1, &text[body_start..end]));
        }
    }
    bodies
}

/// A captured name that holds a path, such as `{path}` or `{database_path}`.
fn captures_a_path(body: &str) -> bool {
    body.split('{').skip(1).any(|rest| {
        let name = rest.split(['}', ':']).next().unwrap_or("");
        let is_name = !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_');
        is_name && ["path", "folder", "file", "dir"].iter().any(|word| name.contains(word))
    })
}

fn problem_with(body: &str) -> Option<&'static str> {
    if body.contains(".display()") {
        Some("prints a path with .display()")
    } else if body.contains("to_string_lossy") {
        Some("prints a path with to_string_lossy")
    } else if captures_a_path(body) {
        Some("captures a path by name")
    } else if body.contains(".path)")
        || body.contains(".path,")
        || body.trim_end().ends_with(".path")
    {
        // `place(&open.path)` is the way, and it ends `.path)` too.
        (!body.contains("place(")).then_some("prints a path field")
    } else {
        None
    }
}

#[test]
fn no_log_macro_prints_a_path_except_as_a_shape() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&root, &mut files);
    assert!(files.len() > 50, "the walk found the source: {}", files.len());
    let mut problems = Vec::new();
    let mut macros = 0;
    for file in &files {
        let text = fs::read_to_string(file).unwrap();
        for (line, body) in macro_bodies(&text) {
            macros += 1;
            if let Some(problem) = problem_with(body) {
                problems.push(format!("{}:{line}: {problem}", file.display()));
            }
        }
    }
    assert!(macros > 150, "the walk found the log macros: {macros}");
    assert!(problems.is_empty(), "use logging::place:\n{}", problems.join("\n"));
}

/// The check itself: each way of naming a path is caught, and the ways that
/// are allowed aren't.
#[test]
fn the_check_catches_each_way_of_printing_a_path() {
    let flagged = |body: &str| problem_with(body).is_some();
    assert!(flagged(r#""could not open {}", path.display()"#));
    assert!(flagged(r#""could not open {}", path.to_string_lossy()"#));
    assert!(flagged(r#""could not open {path}""#));
    assert!(flagged(r#""could not open {database_path:?}""#));
    assert!(flagged(r#""could not open {}", open.path"#));
    assert!(!flagged(r#""could not open {}", crate::logging::place(&open.path)"#));
    assert!(!flagged(r#""could not open a file: {err}""#));
    assert!(!flagged(r#""{} {{literal}}", code"#));
}
