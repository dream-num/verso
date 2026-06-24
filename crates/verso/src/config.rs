use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub version: VersionConfig,
    pub workspaces: WorkspaceConfig,
    pub changelog: ChangelogConfig,
    pub git: GitConfig,
    pub github_release: GithubReleaseConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionConfig {
    pub root_package: String,
    pub require_consistent_versions: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceConfig {
    pub patterns: Vec<String>,
    pub include_root: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangelogConfig {
    pub infile: String,
    pub preset: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitConfig {
    pub require_clean_worktree: bool,
    pub commit_message: String,
    pub tag_name: String,
    pub push: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubReleaseConfig {
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    version: Option<RawVersionConfig>,
    workspaces: RawWorkspaceConfig,
    changelog: Option<RawChangelogConfig>,
    git: Option<RawGitConfig>,
    github_release: Option<RawGithubReleaseConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVersionConfig {
    root_package: Option<String>,
    require_consistent_versions: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWorkspaceConfig {
    patterns: Vec<String>,
    include_root: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawChangelogConfig {
    infile: Option<String>,
    preset: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGitConfig {
    require_clean_worktree: Option<bool>,
    commit_message: Option<String>,
    tag_name: Option<String>,
    push: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGithubReleaseConfig {
    enabled: Option<bool>,
}

pub fn load_config(path: &Path) -> Result<Config, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    parse_config(&contents)
}

pub fn parse_config(contents: &str) -> Result<Config, String> {
    let raw: RawConfig =
        toml::from_str(contents).map_err(|error| format!("failed to parse verso.toml: {error}"))?;

    let version = raw.version.unwrap_or(RawVersionConfig {
        root_package: None,
        require_consistent_versions: None,
    });
    let changelog = raw.changelog.unwrap_or(RawChangelogConfig {
        infile: None,
        preset: None,
    });
    let git = raw.git.unwrap_or(RawGitConfig {
        require_clean_worktree: None,
        commit_message: None,
        tag_name: None,
        push: None,
    });
    let github_release = raw
        .github_release
        .unwrap_or(RawGithubReleaseConfig { enabled: None });

    let config = Config {
        version: VersionConfig {
            root_package: version
                .root_package
                .unwrap_or_else(|| "package.json".to_string()),
            require_consistent_versions: version.require_consistent_versions.unwrap_or(true),
        },
        workspaces: WorkspaceConfig {
            patterns: raw.workspaces.patterns,
            include_root: raw.workspaces.include_root.unwrap_or(true),
        },
        changelog: ChangelogConfig {
            infile: changelog
                .infile
                .unwrap_or_else(|| "CHANGELOG.md".to_string()),
            preset: changelog.preset.unwrap_or_else(|| "angular".to_string()),
        },
        git: GitConfig {
            require_clean_worktree: git.require_clean_worktree.unwrap_or(true),
            commit_message: git
                .commit_message
                .unwrap_or_else(|| "chore(release): release v${version}".to_string()),
            tag_name: git.tag_name.unwrap_or_else(|| "v${version}".to_string()),
            push: git.push.unwrap_or_else(|| "follow-tags".to_string()),
        },
        github_release: GithubReleaseConfig {
            enabled: github_release.enabled.unwrap_or(false),
        },
    };

    validate_config(&config)?;
    Ok(config)
}

fn validate_config(config: &Config) -> Result<(), String> {
    if config.workspaces.patterns.is_empty() {
        return Err("workspaces.patterns must contain at least one pattern".to_string());
    }
    if config
        .workspaces
        .patterns
        .iter()
        .any(|pattern| pattern.trim().is_empty())
    {
        return Err("workspaces.patterns must not contain empty patterns".to_string());
    }
    if config.changelog.preset != "angular" {
        return Err("only changelog preset \"angular\" is supported".to_string());
    }
    if config.git.push != "follow-tags" {
        return Err("only git.push = \"follow-tags\" is supported".to_string());
    }
    if config.github_release.enabled {
        return Err("github_release.enabled = true is not supported in this version".to_string());
    }
    Ok(())
}

pub fn render_template(template: &str, version: &str) -> String {
    template.replace("${version}", version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_config_uses_defaults() -> Result<(), String> {
        let config = parse_config(
            r#"
            [workspaces]
            patterns = ["packages/*"]
            "#,
        )?;

        assert_eq!(config.version.root_package, "package.json");
        assert!(config.version.require_consistent_versions);
        assert_eq!(config.workspaces.patterns, vec!["packages/*"]);
        assert!(config.workspaces.include_root);
        assert_eq!(config.changelog.infile, "CHANGELOG.md");
        assert_eq!(config.changelog.preset, "angular");
        assert!(config.git.require_clean_worktree);
        assert_eq!(
            config.git.commit_message,
            "chore(release): release v${version}"
        );
        assert_eq!(config.git.tag_name, "v${version}");
        assert_eq!(config.git.push, "follow-tags");
        assert!(!config.github_release.enabled);

        Ok(())
    }

    #[test]
    fn rejects_enabled_github_release() {
        let error = parse_config(
            r#"
            [workspaces]
            patterns = ["packages/*"]

            [github_release]
            enabled = true
            "#,
        )
        .expect_err("github releases should be unsupported");

        assert!(error.contains("github_release.enabled = true"));
    }

    #[test]
    fn rejects_empty_workspace_patterns_array() {
        let error = parse_config(
            r#"
            [workspaces]
            patterns = []
            "#,
        )
        .expect_err("empty workspace patterns should be rejected");

        assert!(error.contains("workspaces.patterns"));
    }

    #[test]
    fn rejects_blank_workspace_pattern_entries() {
        for pattern in ["", "   "] {
            let contents = format!(
                r#"
                [workspaces]
                patterns = [{pattern:?}]
                "#
            );
            let error =
                parse_config(&contents).expect_err("blank workspace pattern should be rejected");

            assert!(error.contains("workspaces.patterns"));
        }
    }

    #[test]
    fn rejects_invalid_changelog_preset() {
        let error = parse_config(
            r#"
            [workspaces]
            patterns = ["packages/*"]

            [changelog]
            preset = "conventional"
            "#,
        )
        .expect_err("unsupported changelog preset should be rejected");

        assert!(error.contains("changelog preset"));
    }

    #[test]
    fn rejects_invalid_git_push() {
        let error = parse_config(
            r#"
            [workspaces]
            patterns = ["packages/*"]

            [git]
            push = "never"
            "#,
        )
        .expect_err("unsupported git push mode should be rejected");

        assert!(error.contains("git.push"));
    }

    #[test]
    fn rejects_unknown_root_key() {
        let error = parse_config(
            r#"
            unknown = true

            [workspaces]
            patterns = ["packages/*"]
            "#,
        )
        .expect_err("unknown root keys should be rejected");

        assert!(error.contains("unknown field"));
    }

    #[test]
    fn rejects_unknown_nested_key() {
        let error = parse_config(
            r#"
            [workspaces]
            patterns = ["packages/*"]
            typo = true
            "#,
        )
        .expect_err("unknown nested keys should be rejected");

        assert!(error.contains("unknown field"));
    }

    #[test]
    fn renders_version_template() {
        assert_eq!(
            render_template("chore(release): release v${version}", "0.2.0"),
            "chore(release): release v0.2.0"
        );
    }
}
