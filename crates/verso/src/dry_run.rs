use semver::Version;
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleasePlan {
    pub current_version: Version,
    pub target_version: Version,
    pub package_files: Vec<PathBuf>,
    pub extra_version_files: Vec<PathBuf>,
    pub changelog_file: PathBuf,
    pub commit_message: String,
    pub tag_name: String,
    pub hooks: Vec<PlannedHook>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedHook {
    pub name: String,
    pub command: String,
}

pub fn render_dry_run(root: &Path, plan: &ReleasePlan) -> String {
    let mut output = String::new();
    let package_files = normalized_files(&plan.package_files);
    let extra_version_files = normalized_files(&plan.extra_version_files);
    let version_files = normalized_files(
        &package_files
            .iter()
            .chain(extra_version_files.iter())
            .cloned()
            .collect::<Vec<_>>(),
    );

    output.push_str("Verso dry run\n\n");
    output.push_str(&format!("Current version: {}\n", plan.current_version));
    output.push_str(&format!("Target version: {}\n", plan.target_version));
    output.push_str(&format!("Package count: {}\n", package_files.len()));
    if !extra_version_files.is_empty() {
        output.push_str(&format!(
            "Extra version file count: {}\n",
            extra_version_files.len()
        ));
    }

    if !plan.warnings.is_empty() {
        output.push_str("\nWarnings:\n");
        for warning in &plan.warnings {
            output.push_str(&format!("- {warning}\n"));
        }
    }

    output.push_str("\nVersion updates:\n");
    output.push_str(&render_tree(root, &version_files));

    let changelog = relative_path(root, &plan.changelog_file);
    output.push_str(&format!("\nChangelog: {}\n", changelog.display()));

    if !plan.hooks.is_empty() {
        output.push_str("\nPlanned hooks:\n");
        for hook in &plan.hooks {
            output.push_str(&format!("{}: {}\n", hook.name, hook.command));
        }
    }

    let mut git_add_files = version_files;
    git_add_files.push(plan.changelog_file.clone());
    git_add_files.sort();
    git_add_files.dedup();
    let git_add_args = git_add_files
        .iter()
        .map(|file| shell_quote(&relative_path(root, file).display().to_string()))
        .collect::<Vec<_>>()
        .join(" ");

    output.push_str("\nPlanned git commands:\n");
    output.push_str(&format!("git add {git_add_args}\n"));
    output.push_str(&format!(
        "git commit -m {}\n",
        shell_quote(&plan.commit_message)
    ));
    output.push_str(&format!(
        "git tag -a {} -m {}\n",
        shell_quote(&plan.tag_name),
        shell_quote(&plan.tag_name)
    ));
    output.push_str("git push --follow-tags\n");

    output
}

pub fn render_tree(root: &Path, files: &[PathBuf]) -> String {
    let mut tree = TreeNode::default();

    for file in normalized_files(files) {
        let relative = relative_path(root, &file);
        let labels = path_labels(&relative);
        if !labels.is_empty() {
            tree.insert(&labels);
        }
    }

    let mut output = ".\n".to_owned();
    render_children(&tree, "", &mut output);
    output
}

fn normalized_files(files: &[PathBuf]) -> Vec<PathBuf> {
    let mut normalized = files.to_vec();
    normalized.sort();
    normalized.dedup();
    normalized
}

#[derive(Default)]
struct TreeNode {
    children: BTreeMap<String, TreeNode>,
}

impl TreeNode {
    fn insert(&mut self, labels: &[String]) {
        let Some((label, rest)) = labels.split_first() else {
            return;
        };

        self.children.entry(label.clone()).or_default().insert(rest);
    }
}

fn render_children(node: &TreeNode, prefix: &str, output: &mut String) {
    let child_count = node.children.len();
    for (index, (label, child)) in node.children.iter().enumerate() {
        let is_last = index + 1 == child_count;
        let connector = if is_last { "└── " } else { "├── " };
        output.push_str(prefix);
        output.push_str(connector);
        output.push_str(label);
        output.push('\n');

        let child_prefix = if is_last {
            format!("{prefix}    ")
        } else {
            format!("{prefix}│   ")
        };
        render_children(child, &child_prefix, output);
    }
}

fn relative_path(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| path.to_path_buf())
}

fn path_labels(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(label) => Some(label.to_string_lossy().into_owned()),
            Component::ParentDir => Some("..".to_owned()),
            Component::Prefix(prefix) => Some(prefix.as_os_str().to_string_lossy().into_owned()),
            Component::RootDir => Some(std::path::MAIN_SEPARATOR.to_string()),
            Component::CurDir => None,
        })
        .collect()
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use semver::Version;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn renders_package_tree_with_root_package_and_nested_packages() -> Result<(), String> {
        let root = TempDir::new().map_err(|error| error.to_string())?;
        let files = vec![
            root.path().join("package.json"),
            root.path().join("packages/a/package.json"),
            root.path().join("presets/packages/preset/package.json"),
        ];

        let tree = render_tree(root.path(), &files);

        assert_eq!(
            tree,
            concat!(
                ".\n",
                "├── package.json\n",
                "├── packages\n",
                "│   └── a\n",
                "│       └── package.json\n",
                "└── presets\n",
                "    └── packages\n",
                "        └── preset\n",
                "            └── package.json\n",
            )
        );
        Ok(())
    }

    #[test]
    fn dry_run_mentions_git_push_and_warning() -> Result<(), String> {
        let root = TempDir::new().map_err(|error| error.to_string())?;
        let plan = test_plan(
            root.path(),
            vec![root.path().join("package.json")],
            vec!["working tree has generated files".to_owned()],
        )?;

        let output = render_dry_run(root.path(), &plan);

        assert!(output.contains("Verso dry run"));
        assert!(output.contains("Current version: 1.2.3"));
        assert!(output.contains("Target version: 1.3.0"));
        assert!(output.contains("Package count: 1"));
        assert!(output.contains("Warnings"));
        assert!(output.contains("working tree has generated files"));
        assert!(output.contains("git push --follow-tags"));
        Ok(())
    }

    #[test]
    fn dry_run_renders_changelog_path_relative_to_root() -> Result<(), String> {
        let root = TempDir::new().map_err(|error| error.to_string())?;
        let plan = test_plan(
            root.path(),
            vec![root.path().join("packages/a/package.json")],
            Vec::new(),
        )?;

        let output = render_dry_run(root.path(), &plan);

        assert!(output.contains("Changelog: docs/CHANGELOG.md"));
        assert!(!output.contains(&root.path().join("docs/CHANGELOG.md").display().to_string()));
        Ok(())
    }

    #[test]
    fn dry_run_quotes_git_add_paths_with_spaces_and_single_quotes() -> Result<(), String> {
        let root = TempDir::new().map_err(|error| error.to_string())?;
        let plan = test_plan(
            root.path(),
            vec![root.path().join("packages/space dir/bob's/package.json")],
            Vec::new(),
        )?;

        let output = render_dry_run(root.path(), &plan);

        assert!(output
            .contains("git add 'docs/CHANGELOG.md' 'packages/space dir/bob'\\''s/package.json'\n"));
        Ok(())
    }

    #[test]
    fn dry_run_uses_deduped_package_files_for_count_tree_and_git_add() -> Result<(), String> {
        let root = TempDir::new().map_err(|error| error.to_string())?;
        let package = root.path().join("packages/a/package.json");
        let plan = test_plan(root.path(), vec![package.clone(), package], Vec::new())?;

        let output = render_dry_run(root.path(), &plan);

        assert!(output.contains("Package count: 1\n"));
        assert_eq!(output.matches("packages/a/package.json").count(), 1);
        assert!(output.contains("git add 'docs/CHANGELOG.md' 'packages/a/package.json'\n"));
        Ok(())
    }

    #[test]
    fn dry_run_includes_extra_version_files_in_tree_and_git_add() -> Result<(), String> {
        let root = TempDir::new().map_err(|error| error.to_string())?;
        let mut plan = test_plan(
            root.path(),
            vec![root.path().join("packages/verso/package.json")],
            Vec::new(),
        )?;
        plan.extra_version_files = vec![root.path().join("crates/verso/Cargo.toml")];

        let output = render_dry_run(root.path(), &plan);

        assert!(output.contains("Package count: 1\nExtra version file count: 1\n"));
        assert!(output.contains("crates\n│   └── verso\n│       └── Cargo.toml"));
        assert!(output.contains(
            "git add 'crates/verso/Cargo.toml' 'docs/CHANGELOG.md' 'packages/verso/package.json'\n"
        ));
        Ok(())
    }

    #[test]
    fn dry_run_lists_planned_hooks() -> Result<(), String> {
        let root = TempDir::new().map_err(|error| error.to_string())?;
        let mut plan = test_plan(
            root.path(),
            vec![root.path().join("packages/verso/package.json")],
            Vec::new(),
        )?;
        plan.hooks = vec![
            PlannedHook {
                name: "before_version".to_owned(),
                command: "pnpm test".to_owned(),
            },
            PlannedHook {
                name: "after_version".to_owned(),
                command: "pnpm build".to_owned(),
            },
        ];

        let output = render_dry_run(root.path(), &plan);

        assert!(output.contains("Planned hooks:\n"));
        assert!(output.contains("before_version: pnpm test\n"));
        assert!(output.contains("after_version: pnpm build\n"));
        Ok(())
    }

    fn test_plan(
        root: &Path,
        package_files: Vec<PathBuf>,
        warnings: Vec<String>,
    ) -> Result<ReleasePlan, String> {
        Ok(ReleasePlan {
            current_version: Version::parse("1.2.3").map_err(|error| error.to_string())?,
            target_version: Version::parse("1.3.0").map_err(|error| error.to_string())?,
            package_files,
            extra_version_files: Vec::new(),
            changelog_file: root.join("docs/CHANGELOG.md"),
            commit_message: "chore(release): release v1.3.0".to_owned(),
            tag_name: "v1.3.0".to_owned(),
            hooks: Vec::new(),
            warnings,
        })
    }
}
