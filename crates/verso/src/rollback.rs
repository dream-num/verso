use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct ChangeSet {
    originals: HashMap<PathBuf, Option<Vec<u8>>>,
    written: Vec<PathBuf>,
    written_paths: HashSet<PathBuf>,
}

impl ChangeSet {
    pub fn snapshot(paths: &[PathBuf]) -> Result<Self, String> {
        let mut originals = HashMap::new();

        for path in paths {
            originals.insert(path.clone(), original_contents(path)?);
        }

        Ok(Self {
            originals,
            written: Vec::new(),
            written_paths: HashSet::new(),
        })
    }

    pub fn write(&mut self, path: &Path, contents: &[u8]) -> Result<(), String> {
        if !self.originals.contains_key(path) {
            self.originals
                .insert(path.to_path_buf(), original_contents(path)?);
        }

        create_parent_dir(path)?;

        fs::write(path, contents)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;

        let written_path = path.to_path_buf();
        if self.written_paths.insert(written_path.clone()) {
            self.written.push(written_path);
        }

        Ok(())
    }

    /// Restores written paths in reverse write order.
    ///
    /// Returns on the first restore/remove failure, so an error may mean the
    /// rollback was only partially applied.
    pub fn rollback(&self) -> Result<Vec<PathBuf>, String> {
        let mut restored = Vec::new();

        for path in self.written.iter().rev() {
            match self
                .originals
                .get(path)
                .ok_or_else(|| format!("missing rollback snapshot for {}", path.display()))?
            {
                Some(contents) => {
                    create_parent_dir(path)?;
                    fs::write(path, contents).map_err(|error| {
                        format!("failed to restore {}: {error}", path.display())
                    })?;
                }
                None => match fs::remove_file(path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(format!("failed to remove {}: {error}", path.display()));
                    }
                },
            }
            restored.push(path.clone());
        }

        Ok(restored)
    }
}

fn original_contents(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::read(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("failed to snapshot {}: {error}", path.display())),
    }
}

fn create_parent_dir(path: &Path) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };

    if parent.as_os_str().is_empty() {
        return Ok(());
    }

    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn restores_a_written_existing_file() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        let path = temp.path().join("package.json");
        fs::write(&path, b"original").map_err(|error| error.to_string())?;
        let mut changes = ChangeSet::snapshot(std::slice::from_ref(&path))?;

        changes.write(&path, b"changed")?;
        let restored = changes.rollback()?;

        assert_eq!(
            fs::read(&path).map_err(|error| error.to_string())?,
            b"original"
        );
        assert_eq!(restored, vec![path]);
        Ok(())
    }

    #[test]
    fn removes_a_created_file() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        let path = temp.path().join("generated/package.json");
        let mut changes = ChangeSet::snapshot(std::slice::from_ref(&path))?;

        changes.write(&path, b"created")?;
        let restored = changes.rollback()?;

        assert!(!path.exists());
        assert_eq!(restored, vec![path]);
        Ok(())
    }

    #[test]
    fn writing_the_same_file_more_than_once_restores_the_original() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        let path = temp.path().join("package.json");
        fs::write(&path, b"original").map_err(|error| error.to_string())?;
        let mut changes = ChangeSet::snapshot(std::slice::from_ref(&path))?;

        changes.write(&path, b"first change")?;
        changes.write(&path, b"second change")?;
        let restored = changes.rollback()?;

        assert_eq!(
            fs::read(&path).map_err(|error| error.to_string())?,
            b"original"
        );
        assert_eq!(restored, vec![path]);
        Ok(())
    }

    #[test]
    fn restores_multiple_files_in_reverse_write_order() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        let a = temp.path().join("a.txt");
        let b = temp.path().join("b.txt");
        fs::write(&a, b"a original").map_err(|error| error.to_string())?;
        fs::write(&b, b"b original").map_err(|error| error.to_string())?;
        let mut changes = ChangeSet::snapshot(&[a.clone(), b.clone()])?;

        changes.write(&a, b"a changed")?;
        changes.write(&b, b"b changed")?;
        let restored = changes.rollback()?;

        assert_eq!(restored, vec![b, a]);
        Ok(())
    }

    #[test]
    fn rollback_only_affects_paths_that_were_written() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        let written = temp.path().join("written.txt");
        let unwritten = temp.path().join("unwritten.txt");
        fs::write(&written, b"written original").map_err(|error| error.to_string())?;
        fs::write(&unwritten, b"unwritten original").map_err(|error| error.to_string())?;
        let mut changes = ChangeSet::snapshot(&[written.clone(), unwritten.clone()])?;

        changes.write(&written, b"written changed")?;
        fs::write(&unwritten, b"outside change").map_err(|error| error.to_string())?;
        let restored = changes.rollback()?;

        assert_eq!(
            fs::read(&written).map_err(|error| error.to_string())?,
            b"written original"
        );
        assert_eq!(
            fs::read(&unwritten).map_err(|error| error.to_string())?,
            b"outside change"
        );
        assert_eq!(restored, vec![written]);
        Ok(())
    }

    #[test]
    fn preserves_binary_content() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        let path = temp.path().join("artifact.bin");
        let original = vec![0, 159, 146, 150, 255];
        fs::write(&path, &original).map_err(|error| error.to_string())?;
        let mut changes = ChangeSet::snapshot(std::slice::from_ref(&path))?;

        changes.write(&path, &[1, 2, 3, 4])?;
        changes.rollback()?;

        assert_eq!(
            fs::read(&path).map_err(|error| error.to_string())?,
            original
        );
        Ok(())
    }
}
