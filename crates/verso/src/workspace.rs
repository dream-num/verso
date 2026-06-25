use crate::{
    config::Config,
    package_json::{read_package, PackageInfo},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageFile {
    pub dir: PathBuf,
    pub package_json: PathBuf,
    pub info: PackageInfo,
}

pub fn discover_packages(root: &Path, config: &Config) -> Result<Vec<PackageFile>, String> {
    let mut package_paths = Vec::new();
    let mut matched_workspace_package = false;
    let root_package = root.join(&config.version.root_package);

    if config.workspaces.include_root && root_package.exists() {
        package_paths.push(root_package.clone());
    }

    for pattern in &config.workspaces.patterns {
        for dir in expand_workspace_pattern(root, pattern)? {
            let package_json = dir.join("package.json");
            if package_json.exists() {
                if package_json != root_package {
                    matched_workspace_package = true;
                }
                package_paths.push(package_json);
            }
        }
    }

    package_paths.sort();
    package_paths.dedup();

    let mut packages = Vec::with_capacity(package_paths.len());
    for package_json in package_paths {
        let dir = package_json
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", package_json.display()))?
            .to_path_buf();
        let info = read_package(&package_json)?;
        packages.push(PackageFile {
            dir,
            package_json,
            info,
        });
    }

    if packages.is_empty() {
        return Err(format!(
            "no packages discovered under {} from configured workspaces",
            root.display()
        ));
    }

    if !matched_workspace_package {
        return Err(format!(
            "no workspace package.json files matched configured workspaces under {}",
            root.display()
        ));
    }

    Ok(packages)
}

pub fn verify_consistent_versions(packages: &[PackageFile]) -> Result<(), String> {
    let Some(first) = packages.first() else {
        return Ok(());
    };

    let expected = &first.info.version;
    let mismatches: Vec<&PackageFile> = packages
        .iter()
        .filter(|package| package.info.version != *expected)
        .collect();

    if mismatches.is_empty() {
        return Ok(());
    }

    let details = packages
        .iter()
        .map(|package| {
            format!(
                "{} has version {}",
                package_label(package),
                package.info.version
            )
        })
        .collect::<Vec<_>>();

    Err(format!("package versions differ: {}", details.join("; ")))
}

fn expand_workspace_pattern(root: &Path, pattern: &str) -> Result<Vec<PathBuf>, String> {
    let wildcard_count = pattern.matches('*').count();
    match wildcard_count {
        0 => Ok(vec![root.join(pattern)]),
        1 => expand_one_wildcard_pattern(root, pattern),
        _ => Err(format!(
            "workspace pattern {pattern:?} contains more than one wildcard"
        )),
    }
}

fn expand_one_wildcard_pattern(root: &Path, pattern: &str) -> Result<Vec<PathBuf>, String> {
    let (prefix, suffix) = pattern
        .split_once('*')
        .ok_or_else(|| format!("workspace pattern {pattern:?} did not contain a wildcard"))?;
    let prefix = prefix.trim_end_matches('/');
    let suffix = suffix.trim_start_matches('/');
    let base = if prefix.is_empty() {
        root.to_path_buf()
    } else {
        root.join(prefix)
    };

    let entries = match fs::read_dir(&base) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "failed to read workspace pattern {pattern:?} at {}: {error}",
                base.display()
            ));
        }
    };

    let mut dirs = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read workspace pattern {pattern:?} at {}: {error}",
                base.display()
            )
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let candidate = if suffix.is_empty() {
            path
        } else {
            path.join(suffix)
        };
        if candidate.is_dir() {
            dirs.push(candidate);
        }
    }

    Ok(dirs)
}

fn package_label(package: &PackageFile) -> String {
    match &package.info.name {
        Some(name) => format!("{name} ({})", package.package_json.display()),
        None => package.package_json.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        ChangelogConfig, Config, GitConfig, GithubReleaseConfig, VersionConfig, WorkspaceConfig,
    };
    use std::{fs, path::Path};
    use tempfile::TempDir;

    #[test]
    fn discovers_root_and_workspace_packages() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        write_package(temp.path(), "root", "1.2.3")?;
        write_package(&temp.path().join("packages/a"), "a", "1.2.3")?;
        write_package(&temp.path().join("apps/web"), "web", "1.2.3")?;
        fs::create_dir_all(temp.path().join("packages/empty"))
            .map_err(|error| error.to_string())?;
        let config = test_config(vec!["packages/*", "apps/*"], true);

        let packages = discover_packages(temp.path(), &config)?;

        assert_package_dirs(
            &packages,
            &[
                &temp.path().join("apps/web"),
                temp.path(),
                &temp.path().join("packages/a"),
            ],
        );
        verify_consistent_versions(&packages)?;
        Ok(())
    }

    #[test]
    fn detects_inconsistent_versions() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        write_package(temp.path(), "root", "1.2.3")?;
        write_package(&temp.path().join("packages/a"), "a", "1.2.4")?;
        let config = test_config(vec!["packages/*"], true);
        let packages = discover_packages(temp.path(), &config)?;

        let error = verify_consistent_versions(&packages)
            .expect_err("version mismatch should return an error");

        assert!(error.contains("root"));
        assert!(error.contains("a"));
        assert!(error.contains("1.2.3"));
        assert!(error.contains("1.2.4"));
        Ok(())
    }

    #[test]
    fn mismatch_message_lists_versions_without_arbitrary_expected_baseline() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        write_package(temp.path(), "root", "1.2.3")?;
        write_package(&temp.path().join("apps/web"), "web", "1.2.4")?;
        let config = test_config(vec!["apps/*"], true);
        let packages = discover_packages(temp.path(), &config)?;

        let error = verify_consistent_versions(&packages)
            .expect_err("version mismatch should return an error");

        assert!(error.contains("root"));
        assert!(error.contains("web"));
        assert!(error.contains("1.2.3"));
        assert!(error.contains("1.2.4"));
        assert!(!error.contains("expected all packages to use 1.2.4"));
        Ok(())
    }

    #[test]
    fn discovers_nested_prefix_workspace_pattern() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        write_package(&temp.path().join("presets/packages/foo"), "foo", "1.2.3")?;
        let config = test_config(vec!["presets/packages/*"], false);

        let packages = discover_packages(temp.path(), &config)?;

        assert_package_dirs(&packages, &[&temp.path().join("presets/packages/foo")]);
        Ok(())
    }

    #[test]
    fn skips_root_when_include_root_is_false() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        write_package(temp.path(), "root", "1.2.3")?;
        write_package(&temp.path().join("packages/a"), "a", "1.2.3")?;
        let config = test_config(vec!["packages/*"], false);

        let packages = discover_packages(temp.path(), &config)?;

        assert_package_dirs(&packages, &[&temp.path().join("packages/a")]);
        Ok(())
    }

    #[test]
    fn errors_when_root_exists_but_no_workspace_packages_match() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        write_package(temp.path(), "root", "1.2.3")?;
        fs::create_dir_all(temp.path().join("packages/empty"))
            .map_err(|error| error.to_string())?;
        let config = test_config(vec!["packages/*"], true);

        let error = discover_packages(temp.path(), &config)
            .expect_err("root package alone should not satisfy workspace discovery");

        assert!(error.contains("no workspace package.json files matched configured workspaces"));
        Ok(())
    }

    #[test]
    fn deduplicates_duplicate_workspace_patterns() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        write_package(&temp.path().join("packages/a"), "a", "1.2.3")?;
        let config = test_config(vec!["packages/*", "packages/*"], false);

        let packages = discover_packages(temp.path(), &config)?;

        assert_eq!(packages.len(), 1);
        assert_package_dirs(&packages, &[&temp.path().join("packages/a")]);
        Ok(())
    }

    #[test]
    fn errors_when_no_packages_are_discovered() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        fs::create_dir_all(temp.path().join("packages/empty"))
            .map_err(|error| error.to_string())?;
        let config = test_config(vec!["packages/*"], false);

        let error =
            discover_packages(temp.path(), &config).expect_err("empty discovery should fail");

        assert!(error.contains("no packages"));
        Ok(())
    }

    #[test]
    fn discovers_minimal_univer_pro_workspace_shape() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        write_package(&temp.path().join("apps/docs"), "docs", "1.2.3")?;
        write_package(&temp.path().join("bundle/core"), "core-bundle", "1.2.3")?;
        write_package(&temp.path().join("packages/sheets"), "sheets", "1.2.3")?;
        write_package(
            &temp.path().join("packages-experimental/labs"),
            "labs",
            "1.2.3",
        )?;
        write_package(
            &temp.path().join("presets/packages/basic"),
            "basic",
            "1.2.3",
        )?;
        let config = test_config(
            vec![
                "apps/*",
                "bundle/*",
                "packages/*",
                "packages-experimental/*",
                "presets/packages/*",
            ],
            false,
        );

        let packages = discover_packages(temp.path(), &config)?;

        assert_package_dirs(
            &packages,
            &[
                &temp.path().join("apps/docs"),
                &temp.path().join("bundle/core"),
                &temp.path().join("packages/sheets"),
                &temp.path().join("packages-experimental/labs"),
                &temp.path().join("presets/packages/basic"),
            ],
        );
        Ok(())
    }

    fn test_config(patterns: Vec<&str>, include_root: bool) -> Config {
        Config {
            version: VersionConfig {
                root_package: "package.json".to_owned(),
                require_consistent_versions: true,
                cargo_manifest_paths: Vec::new(),
            },
            workspaces: WorkspaceConfig {
                patterns: patterns.into_iter().map(ToOwned::to_owned).collect(),
                include_root,
            },
            changelog: ChangelogConfig {
                infile: "CHANGELOG.md".to_owned(),
                preset: "angular".to_owned(),
            },
            git: GitConfig {
                require_clean_worktree: true,
                commit_message: "chore(release): release v${version}".to_owned(),
                tag_name: "v${version}".to_owned(),
                push: "follow-tags".to_owned(),
            },
            github_release: GithubReleaseConfig { enabled: false },
        }
    }

    fn write_package(dir: &Path, name: &str, version: &str) -> Result<(), String> {
        fs::create_dir_all(dir).map_err(|error| error.to_string())?;
        fs::write(
            dir.join("package.json"),
            format!(r#"{{"name":"{name}","version":"{version}"}}"#),
        )
        .map_err(|error| error.to_string())
    }

    fn assert_package_dirs(packages: &[PackageFile], expected: &[&Path]) {
        let actual: Vec<&Path> = packages
            .iter()
            .map(|package| package.dir.as_path())
            .collect();
        assert_eq!(actual, expected);
    }
}
