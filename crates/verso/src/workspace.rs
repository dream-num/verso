use crate::{
    config::Config,
    package_json::{read_package, PackageInfo},
};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::{
    collections::BTreeSet,
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
    let workspace_patterns = WorkspacePatterns::new(&config.workspaces.patterns)?;

    if config.workspaces.include_root && root_package.exists() {
        package_paths.push(root_package.clone());
    }

    for search_root in workspace_search_roots(root, &config.workspaces.patterns) {
        for dir in collect_dirs(&search_root)? {
            if !workspace_patterns.is_match(root, &dir) {
                continue;
            }
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

struct WorkspacePatterns {
    includes: GlobSet,
    excludes: GlobSet,
}

impl WorkspacePatterns {
    fn new(patterns: &[String]) -> Result<Self, String> {
        let mut includes = GlobSetBuilder::new();
        let mut excludes = GlobSetBuilder::new();

        for pattern in patterns {
            let (excluded, pattern) = match pattern.strip_prefix('!') {
                Some(pattern) => (true, pattern),
                None => (false, pattern.as_str()),
            };
            let glob = GlobBuilder::new(pattern)
                .literal_separator(true)
                .build()
                .map_err(|error| format!("invalid workspace pattern {pattern:?}: {error}"))?;
            if excluded {
                excludes.add(glob);
            } else {
                includes.add(glob);
            }
        }

        Ok(Self {
            includes: includes
                .build()
                .map_err(|error| format!("failed to build workspace include patterns: {error}"))?,
            excludes: excludes
                .build()
                .map_err(|error| format!("failed to build workspace exclude patterns: {error}"))?,
        })
    }

    fn is_match(&self, root: &Path, dir: &Path) -> bool {
        let Ok(relative) = dir.strip_prefix(root) else {
            return false;
        };
        self.includes.is_match(relative) && !self.excludes.is_match(relative)
    }
}

fn workspace_search_roots(root: &Path, patterns: &[String]) -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();
    for pattern in patterns {
        if pattern.starts_with('!') {
            continue;
        }
        let prefix = static_pattern_prefix(pattern);
        let search_root = if prefix.is_empty() {
            root.to_path_buf()
        } else {
            root.join(prefix)
        };
        roots.insert(search_root);
    }
    roots.into_iter().collect()
}

fn static_pattern_prefix(pattern: &str) -> String {
    let glob_start = pattern
        .char_indices()
        .find_map(|(index, character)| matches!(character, '*' | '?' | '[' | '{').then_some(index))
        .unwrap_or(pattern.len());
    let prefix = &pattern[..glob_start];
    match prefix.rsplit_once('/') {
        Some((parent, _segment)) => parent.trim_end_matches('/').to_owned(),
        None if glob_start == pattern.len() => pattern.trim_end_matches('/').to_owned(),
        None => String::new(),
    }
}

fn collect_dirs(root: &Path) -> Result<Vec<PathBuf>, String> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    if !root.is_dir() {
        return Ok(Vec::new());
    }

    let mut dirs = vec![root.to_path_buf()];
    let entries = fs::read_dir(root).map_err(|error| {
        format!(
            "failed to read workspace directory {}: {error}",
            root.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read workspace directory {}: {error}",
                root.display()
            )
        })?;
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "node_modules") {
                continue;
            }
            dirs.extend(collect_dirs(&path)?);
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
        ChangelogConfig, Config, GitConfig, GithubReleaseConfig, HooksConfig, VersionConfig,
        WorkspaceConfig,
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
    fn discovers_packages_with_recursive_workspace_globs() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        write_package(&temp.path().join("packages/a"), "a", "1.2.3")?;
        write_package(&temp.path().join("packages/nested/b"), "b", "1.2.3")?;
        let config = test_config(vec!["packages/**"], false);

        let packages = discover_packages(temp.path(), &config)?;

        assert_package_dirs(
            &packages,
            &[
                &temp.path().join("packages/a"),
                &temp.path().join("packages/nested/b"),
            ],
        );
        Ok(())
    }

    #[test]
    fn excludes_packages_with_negative_workspace_globs() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        write_package(&temp.path().join("packages/a"), "a", "1.2.3")?;
        write_package(&temp.path().join("packages/demo"), "demo", "1.2.3")?;
        write_package(
            &temp.path().join("packages/nested/fixture"),
            "fixture",
            "1.2.3",
        )?;
        let config = test_config(
            vec!["packages/**", "!packages/demo", "!packages/**/fixture"],
            false,
        );

        let packages = discover_packages(temp.path(), &config)?;

        assert_package_dirs(&packages, &[&temp.path().join("packages/a")]);
        Ok(())
    }

    #[test]
    fn recursive_workspace_globs_ignore_node_modules() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        write_package(&temp.path().join("packages/a"), "a", "1.2.3")?;
        write_package(
            &temp.path().join("packages/a/node_modules/dependency"),
            "dependency",
            "9.9.9",
        )?;
        let config = test_config(vec!["packages/**"], false);

        let packages = discover_packages(temp.path(), &config)?;

        assert_package_dirs(&packages, &[&temp.path().join("packages/a")]);
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
    fn single_star_workspace_globs_do_not_cross_path_segments() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        write_package(&temp.path().join("packages/a"), "a", "1.2.3")?;
        write_package(
            &temp.path().join("packages/a/node_modules/dependency"),
            "dependency",
            "9.9.9",
        )?;
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
            hooks: HooksConfig::default(),
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
