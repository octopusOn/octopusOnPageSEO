use std::io;
use std::path::{Path, PathBuf};

/// Expands directories into the files they contain (recursively, sorted, hidden entries
/// skipped). Plain file arguments are kept in the given order.
pub(crate) fn expand_inputs(paths: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for path in paths {
        if path.is_dir() {
            collect_dir(path, &mut files)?;
        } else {
            files.push(path.clone());
        }
    }
    Ok(files)
}

fn collect_dir(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    let mut entries = std::fs::read_dir(dir)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<_>>>()?;
    entries.sort();
    for path in entries {
        let hidden = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with('.'));
        if hidden {
            continue;
        }
        if path.is_dir() {
            collect_dir(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_directories_recursively_and_skips_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir(root.join("nested")).unwrap();
        for name in ["b.log", "a.log.gz", ".DS_Store", "nested/c.log"] {
            std::fs::write(root.join(name), "").unwrap();
        }
        let explicit = root.join("b.log");
        let files = expand_inputs(&[explicit.clone(), root.to_owned()]).unwrap();
        assert_eq!(
            files,
            [
                explicit,
                root.join("a.log.gz"),
                root.join("b.log"),
                root.join("nested/c.log")
            ]
        );
    }
}
