use crate::{
    changelog::{self, render_changelog_entry},
    cli::Cli,
    config::{self, render_template},
    dry_run::{render_dry_run, ReleasePlan},
    git, package_json,
    rollback::ChangeSet,
    versioning::{bump_prerelease, bump_stable, parse_custom_version, BaseBump, PrereleaseChannel},
    workspace::{discover_packages, verify_consistent_versions, PackageFile},
};
use semver::Version;
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

pub fn run(cli: Cli) -> Result<(), String> {
    let config_path = PathBuf::from(&cli.config);
    let root = release_root(&config_path)?;
    let config = config::load_config(&config_path)?;
    let packages = discover_packages(&root, &config)?;

    if config.version.require_consistent_versions {
        verify_consistent_versions(&packages)?;
    }

    let current_version =
        current_version(&root, Path::new(&config.version.root_package), &packages)?;
    let target_version =
        resolve_target_version(cli.target_version.as_deref(), &current_version, cli.yes)?;
    let tag_name = render_template(&config.git.tag_name, &target_version.to_string());
    let commit_message = render_template(&config.git.commit_message, &target_version.to_string());
    let changelog_file = root.join(&config.changelog.infile);
    let package_files = packages
        .iter()
        .map(|package| package.package_json.clone())
        .collect::<Vec<_>>();

    if cli.dry_run {
        let warnings = dry_run_warnings(&root, &tag_name)?;
        let plan = ReleasePlan {
            current_version,
            target_version,
            package_files,
            changelog_file,
            commit_message,
            tag_name,
            warnings,
        };
        print!("{}", render_dry_run(&root, &plan));
        return Ok(());
    }

    if config.git.require_clean_worktree && !git::is_worktree_clean(&root)? {
        return Err("working tree is dirty".to_string());
    }
    if git::tag_exists(&root, &tag_name)? {
        return Err(format!("tag {tag_name} already exists"));
    }

    let previous_tag = previous_tag(&root, &config.git.tag_name)?;
    let commits = changelog::commits_since(&root, previous_tag.as_deref())?;
    let repo_slug =
        git::remote_origin_url(&root).and_then(|remote| changelog::infer_github_slug(&remote));
    let changelog_entry = render_changelog_entry(
        &target_version.to_string(),
        previous_tag.as_deref(),
        &tag_name,
        &commits,
        repo_slug.as_deref(),
    );

    let before_head = git::current_head(&root)?;
    let release_files = write_release_files(
        &packages,
        &changelog_file,
        &target_version,
        &changelog_entry,
    )?;
    if let Err(error) = git_add_release_files(&root, &release_files.changed_paths) {
        return Err(rollback_add_failure(&root, &release_files, error));
    }
    if let Err(error) = git::git(&root, &["commit", "-m", &commit_message]) {
        return Err(rollback_commit_failure(&root, &release_files, error));
    }
    let release_head = git::current_head(&root)?;
    if let Err(error) = git::git(&root, &["tag", &tag_name]) {
        return Err(rollback_tag_failure(
            &root,
            &release_files,
            &before_head,
            &release_head,
            error,
        ));
    }
    git::git(&root, &["push", "--follow-tags"]).map_err(|error| {
        format!(
            "{error}\nLocal release commit and tag were created. Fix the remote problem and rerun: git push --follow-tags"
        )
    })?;

    Ok(())
}

fn resolve_target_version(
    input: Option<&str>,
    current: &Version,
    assume_yes: bool,
) -> Result<Version, String> {
    match input {
        Some(version) => {
            let target = parse_custom_version(version)?;
            confirm_non_forward_version(current, &target, assume_yes)?;
            Ok(target)
        }
        None => prompt_target_version(current, assume_yes),
    }
}

fn prompt_target_version(current: &Version, assume_yes: bool) -> Result<Version, String> {
    loop {
        let patch = bump_stable(current, BaseBump::Patch);
        let minor = bump_stable(current, BaseBump::Minor);
        let major = bump_stable(current, BaseBump::Major);

        println!("Select target version:");
        println!("  1) patch ({patch})");
        println!("  2) minor ({minor})");
        println!("  3) major ({major})");
        println!("  4) alpha");
        println!("  5) beta");
        println!("  6) rc");
        println!("  7) custom semver");

        match read_prompt("Choice: ")?.as_str() {
            "1" | "patch" => return Ok(patch),
            "2" | "minor" => return Ok(minor),
            "3" | "major" => return Ok(major),
            "4" | "alpha" => return prompt_prerelease_version(current, PrereleaseChannel::Alpha),
            "5" | "beta" => return prompt_prerelease_version(current, PrereleaseChannel::Beta),
            "6" | "rc" => return prompt_prerelease_version(current, PrereleaseChannel::Rc),
            "7" | "custom" => {
                let target = parse_custom_version(&read_prompt("Version: ")?)?;
                confirm_non_forward_version(current, &target, assume_yes)?;
                return Ok(target);
            }
            _ => println!("Please choose patch, minor, major, alpha, beta, rc, or custom."),
        }
    }
}

fn confirm_non_forward_version(
    current: &Version,
    target: &Version,
    assume_yes: bool,
) -> Result<(), String> {
    if target > current || assume_yes {
        return Ok(());
    }

    let answer =
        read_prompt("Target version is not greater than current version. Continue? [y/N] ")?;
    if matches!(answer.as_str(), "y" | "Y" | "yes" | "YES" | "Yes") {
        Ok(())
    } else {
        Err("release aborted".to_string())
    }
}

fn prompt_prerelease_version(
    current: &Version,
    channel: PrereleaseChannel,
) -> Result<Version, String> {
    loop {
        let patch = bump_prerelease(current, BaseBump::Patch, channel);
        let minor = bump_prerelease(current, BaseBump::Minor, channel);
        let major = bump_prerelease(current, BaseBump::Major, channel);
        let channel = prerelease_channel_label(channel);

        println!("Select {channel} base:");
        println!("  1) patch ({patch})");
        println!("  2) minor ({minor})");
        println!("  3) major ({major})");

        match read_prompt("Choice: ")?.as_str() {
            "1" | "patch" => return Ok(patch),
            "2" | "minor" => return Ok(minor),
            "3" | "major" => return Ok(major),
            _ => println!("Please choose patch, minor, or major."),
        }
    }
}

fn prerelease_channel_label(channel: PrereleaseChannel) -> &'static str {
    match channel {
        PrereleaseChannel::Alpha => "alpha",
        PrereleaseChannel::Beta => "beta",
        PrereleaseChannel::Rc => "rc",
    }
}

fn read_prompt(prompt: &str) -> Result<String, String> {
    print!("{prompt}");
    io::stdout()
        .flush()
        .map_err(|error| format!("failed to flush prompt: {error}"))?;

    let mut input = String::new();
    let bytes = io::stdin()
        .read_line(&mut input)
        .map_err(|error| format!("failed to read prompt input: {error}"))?;
    if bytes == 0 {
        return Err("interactive version selection requires input".to_string());
    }

    Ok(input.trim().to_string())
}

fn release_root(config_path: &Path) -> Result<PathBuf, String> {
    let current_dir =
        std::env::current_dir().map_err(|error| format!("failed to read current dir: {error}"))?;

    let Some(parent) = config_path.parent() else {
        return Ok(current_dir);
    };
    if parent.as_os_str().is_empty() {
        return Ok(current_dir);
    }
    if parent.is_absolute() {
        Ok(parent.to_path_buf())
    } else {
        Ok(current_dir.join(parent))
    }
}

fn current_version(
    root: &Path,
    root_package: &Path,
    packages: &[PackageFile],
) -> Result<Version, String> {
    let root_package = root.join(root_package);
    if let Some(package) = packages
        .iter()
        .find(|package| package.package_json == root_package)
    {
        return Ok(package.info.version.clone());
    }

    packages
        .first()
        .map(|package| package.info.version.clone())
        .ok_or_else(|| "no package version discovered".to_string())
}

fn dry_run_warnings(root: &Path, tag_name: &str) -> Result<Vec<String>, String> {
    let mut warnings = Vec::new();

    if !git::is_worktree_clean(root)? {
        warnings.push("working tree is dirty".to_string());
    }
    if git::tag_exists(root, tag_name)? {
        warnings.push(format!("tag {tag_name} already exists"));
    }

    Ok(warnings)
}

fn previous_tag(root: &Path, tag_template: &str) -> Result<Option<String>, String> {
    let Some((prefix, suffix)) = tag_template.split_once("${version}") else {
        return Ok(None);
    };

    let output = git::git(root, &["tag", "--merged", "HEAD", "--list"])?;
    Ok(output
        .stdout
        .lines()
        .map(str::trim)
        .filter_map(|tag| {
            let version = tag
                .strip_prefix(prefix)?
                .strip_suffix(suffix)
                .and_then(|version| Version::parse(version).ok())?;
            Some((version, tag.to_string()))
        })
        .max_by(|(left_version, _), (right_version, _)| left_version.cmp(right_version))
        .map(|(_version, tag)| tag))
}

struct ReleaseFileChanges {
    changed_paths: Vec<PathBuf>,
    change_set: ChangeSet,
}

fn write_release_files(
    packages: &[PackageFile],
    changelog_file: &Path,
    target_version: &Version,
    changelog_entry: &str,
) -> Result<ReleaseFileChanges, String> {
    let mut paths = packages
        .iter()
        .map(|package| package.package_json.clone())
        .collect::<Vec<_>>();
    paths.push(changelog_file.to_path_buf());

    let mut changes = ChangeSet::snapshot(&paths)?;
    let result = (|| {
        let mut changed_paths = Vec::new();
        for package in packages {
            let contents = fs::read_to_string(&package.package_json).map_err(|error| {
                format!("failed to read {}: {error}", package.package_json.display())
            })?;
            let updated =
                package_json::replace_version_preserving_style(&contents, target_version)?;
            if updated != contents {
                changes.write(&package.package_json, updated.as_bytes())?;
                changed_paths.push(package.package_json.clone());
            }
        }

        let existing_changelog = read_changelog(changelog_file)?;
        let changelog = insert_changelog_entry(&existing_changelog, changelog_entry);
        if changelog != existing_changelog {
            changes.write(changelog_file, changelog.as_bytes())?;
            changed_paths.push(changelog_file.to_path_buf());
        }

        Ok(changed_paths)
    })();

    match result {
        Ok(changed_paths) => Ok(ReleaseFileChanges {
            changed_paths,
            change_set: changes,
        }),
        Err(error) => match changes.rollback() {
            Ok(_restored) => Err(error),
            Err(rollback_error) => Err(format!("{error}; rollback failed: {rollback_error}")),
        },
    }
}

fn read_changelog(path: &Path) -> Result<String, String> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok("# Changelog\n".to_string())
        }
        Err(error) => Err(format!("failed to read {}: {error}", path.display())),
    }
}

fn insert_changelog_entry(existing: &str, entry: &str) -> String {
    let entry = entry.trim_end();

    let (first_line, body) = existing
        .split_once('\n')
        .map_or((existing, ""), |(first_line, body)| (first_line, body));

    if first_line.trim_end() == "# Changelog" {
        let heading = "# Changelog";
        let body = body
            .strip_prefix('\r')
            .unwrap_or(body)
            .trim_start_matches('\n');

        if body.is_empty() {
            format!("{heading}\n\n{entry}\n")
        } else {
            format!("{heading}\n\n{entry}\n\n{body}")
        }
    } else if existing.trim().is_empty() {
        format!("# Changelog\n\n{entry}\n")
    } else {
        format!("# Changelog\n\n{entry}\n\n{existing}")
    }
}

fn git_add_release_files(root: &Path, files: &[PathBuf]) -> Result<(), String> {
    if files.is_empty() {
        return Ok(());
    }

    let mut files = files.to_vec();
    files.sort();
    files.dedup();

    let mut args = vec!["add".to_string(), "--".to_string()];
    args.extend(
        files
            .iter()
            .map(|file| relative_path(root, file).display().to_string()),
    );
    let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();

    git::git(root, &arg_refs)?;
    Ok(())
}

fn rollback_commit_failure(
    root: &Path,
    release_files: &ReleaseFileChanges,
    error: String,
) -> String {
    let paths = relative_path_strings(root, &release_files.changed_paths);
    let unstage_result = git::unstage_paths(root, &paths);
    let rollback_result = release_files.change_set.rollback();

    append_best_effort_errors(error, unstage_result, rollback_result)
}

fn rollback_add_failure(root: &Path, release_files: &ReleaseFileChanges, error: String) -> String {
    let paths = relative_path_strings(root, &release_files.changed_paths);
    let unstage_result = git::unstage_paths(root, &paths);
    let rollback_result = release_files.change_set.rollback();

    append_best_effort_errors(error, unstage_result, rollback_result)
}

fn rollback_tag_failure(
    root: &Path,
    release_files: &ReleaseFileChanges,
    before_head: &str,
    release_head: &str,
    error: String,
) -> String {
    match git::current_head(root) {
        Ok(current_head) if current_head == release_head => {
            let reset_result = git::reset_soft(root, before_head);
            let paths = relative_path_strings(root, &release_files.changed_paths);
            let unstage_result = git::unstage_paths(root, &paths);
            let rollback_result = release_files.change_set.rollback();

            append_tag_rollback_errors(error, reset_result, unstage_result, rollback_result)
        }
        Ok(current_head) => {
            let rollback_result = release_files.change_set.rollback();
            let rollback_note = match rollback_result {
                Ok(_restored) => String::new(),
                Err(rollback_error) => format!("; file rollback failed: {rollback_error}"),
            };
            format!(
                "{error}; HEAD moved unexpectedly from release commit {release_head} to {current_head}; skipped git reset{rollback_note}"
            )
        }
        Err(head_error) => {
            let rollback_result = release_files.change_set.rollback();
            let rollback_note = match rollback_result {
                Ok(_restored) => String::new(),
                Err(rollback_error) => format!("; file rollback failed: {rollback_error}"),
            };
            format!(
                "{error}; failed to verify HEAD before rollback: {head_error}; skipped git reset{rollback_note}"
            )
        }
    }
}

fn append_best_effort_errors(
    error: String,
    unstage_result: Result<(), String>,
    rollback_result: Result<Vec<PathBuf>, String>,
) -> String {
    let mut message = error;
    if let Err(unstage_error) = unstage_result {
        message.push_str(&format!("; unstage failed: {unstage_error}"));
    }
    if let Err(rollback_error) = rollback_result {
        message.push_str(&format!("; rollback failed: {rollback_error}"));
    }
    message
}

fn append_tag_rollback_errors(
    error: String,
    reset_result: Result<(), String>,
    unstage_result: Result<(), String>,
    rollback_result: Result<Vec<PathBuf>, String>,
) -> String {
    let mut message = error;
    if let Err(reset_error) = reset_result {
        message.push_str(&format!("; soft reset failed: {reset_error}"));
    }
    if let Err(unstage_error) = unstage_result {
        message.push_str(&format!("; unstage failed: {unstage_error}"));
    }
    if let Err(rollback_error) = rollback_result {
        message.push_str(&format!("; rollback failed: {rollback_error}"));
    }
    message
}

fn relative_path_strings(root: &Path, paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| relative_path(root, path).display().to_string())
        .collect()
}

fn relative_path(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_error| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package_json::PackageInfo;
    use tempfile::TempDir;

    #[test]
    fn previous_tag_matches_template_prefix_and_suffix() -> Result<(), String> {
        let repo = init_repo()?;
        fs::write(repo.path().join("README.md"), "hello").map_err(|error| error.to_string())?;
        git::git(repo.path(), &["add", "README.md"])?;
        git::git(repo.path(), &["commit", "-m", "feat: initial"])?;
        git::git(repo.path(), &["tag", "pkg-0.1.0-release"])?;
        git::git(repo.path(), &["tag", "pkg-0.2.0-release"])?;
        git::git(repo.path(), &["tag", "pkg-9.9.9-other"])?;

        assert_eq!(
            previous_tag(repo.path(), "pkg-${version}-release")?,
            Some("pkg-0.2.0-release".to_string())
        );

        Ok(())
    }

    #[test]
    fn current_version_prefers_root_package() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        let root_package = test_package(temp.path(), None, "root", "1.2.3")?;
        let workspace_package =
            test_package(temp.path(), Some(Path::new("packages/a")), "a", "9.9.9")?;

        assert_eq!(
            current_version(
                temp.path(),
                Path::new("package.json"),
                &[workspace_package, root_package]
            )?,
            Version::parse("1.2.3").expect("test semver should parse")
        );

        Ok(())
    }

    #[test]
    fn write_release_files_returns_only_changed_paths() -> Result<(), String> {
        let temp = TempDir::new().map_err(|error| error.to_string())?;
        let root_package = test_package(temp.path(), None, "root", "0.2.0")?;
        let workspace_package =
            test_package(temp.path(), Some(Path::new("packages/a")), "a", "0.1.0")?;
        let changelog = temp.path().join("CHANGELOG.md");
        fs::write(&changelog, "# Changelog\n").map_err(|error| error.to_string())?;

        let changed = write_release_files(
            &[root_package.clone(), workspace_package.clone()],
            &changelog,
            &Version::parse("0.2.0").expect("test semver should parse"),
            "# 0.2.0 (2026-06-24)\n\nNo classifiable changes.\n",
        )?;

        assert_eq!(
            changed.changed_paths,
            vec![workspace_package.package_json.clone(), changelog.clone()]
        );
        assert_eq!(
            fs::read_to_string(&root_package.package_json).map_err(|error| error.to_string())?,
            "{\n  \"name\": \"root\",\n  \"version\": \"0.2.0\"\n}\n"
        );

        Ok(())
    }

    #[test]
    fn changelog_insertion_normalizes_heading_whitespace() -> Result<(), String> {
        let updated = insert_changelog_entry("# Changelog   \nold", "# 0.2.0\n\n* feature");
        let crlf_updated = insert_changelog_entry("# Changelog   \r\nold", "# 0.2.0\n\n* feature");

        assert_eq!(updated, "# Changelog\n\n# 0.2.0\n\n* feature\n\nold");
        assert_eq!(crlf_updated, "# Changelog\n\n# 0.2.0\n\n* feature\n\nold");
        Ok(())
    }

    fn init_repo() -> Result<TempDir, String> {
        let repo = TempDir::new().map_err(|error| error.to_string())?;
        git::git(repo.path(), &["init"])?;
        git::git(repo.path(), &["config", "user.email", "test@example.com"])?;
        git::git(repo.path(), &["config", "user.name", "Test User"])?;
        Ok(repo)
    }

    fn test_package(
        root: &Path,
        dir: Option<&Path>,
        name: &str,
        version: &str,
    ) -> Result<PackageFile, String> {
        let dir = dir.map_or_else(|| root.to_path_buf(), |dir| root.join(dir));
        fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        let package_json = dir.join("package.json");
        fs::write(
            &package_json,
            format!("{{\n  \"name\": \"{name}\",\n  \"version\": \"{version}\"\n}}\n"),
        )
        .map_err(|error| error.to_string())?;

        Ok(PackageFile {
            dir,
            package_json,
            info: PackageInfo {
                name: Some(name.to_string()),
                version: Version::parse(version).map_err(|error| error.to_string())?,
            },
        })
    }
}
