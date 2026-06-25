use assert_cmd::Command;
use predicates::prelude::*;
use std::{fs, path::Path, process::Command as ProcessCommand};
use tempfile::TempDir;

#[test]
fn dry_run_does_not_modify_worktree() -> Result<(), Box<dyn std::error::Error>> {
    let repo = TempDir::new()?;
    init_repo(repo.path())?;

    let root_package = "{\n  \"name\": \"root\",\n  \"version\": \"0.1.0\"\n}\n";
    write_file(&repo.path().join("package.json"), root_package)?;
    write_file(
        &repo.path().join("packages/a/package.json"),
        "{\n  \"name\": \"a\",\n  \"version\": \"0.1.0\"\n}\n",
    )?;
    write_file(
        &repo.path().join("verso.toml"),
        r#"
[workspaces]
patterns = ["packages/*"]
include_root = true
"#,
    )?;
    write_file(&repo.path().join("CHANGELOG.md"), "# Changelog\n")?;

    git(repo.path(), &["add", "."])?;
    git(repo.path(), &["commit", "-m", "chore: initial release"])?;
    git(repo.path(), &["tag", "v0.1.0"])?;
    write_file(&repo.path().join("feature.md"), "feature\n")?;
    git(repo.path(), &["add", "feature.md"])?;
    git(repo.path(), &["commit", "-m", "feat: add feature (#1)"])?;

    Command::cargo_bin("verso")?
        .current_dir(repo.path())
        .args(["--dry-run", "--version", "0.2.0", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Verso dry run"))
        .stdout(predicate::str::contains("Version updates"))
        .stdout(predicate::str::contains("git tag -a 'v0.2.0' -m 'v0.2.0'"))
        .stdout(predicate::str::contains("git push --follow-tags"));

    assert_eq!(
        fs::read_to_string(repo.path().join("package.json"))?,
        root_package
    );

    Ok(())
}

#[test]
fn release_updates_versions_changelog_commit_and_tag_before_push(
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = TempDir::new()?;
    write_release_fixture(repo.path())?;

    Command::cargo_bin("verso")?
        .current_dir(repo.path())
        .args(["--version", "0.2.0", "--yes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Local release commit and tag were created",
        ))
        .stderr(predicate::str::contains("git push --follow-tags"));

    assert!(
        fs::read_to_string(repo.path().join("package.json"))?.contains("\"version\": \"0.2.0\"")
    );
    assert!(
        fs::read_to_string(repo.path().join("packages/a/package.json"))?
            .contains("\"version\": \"0.2.0\"")
    );
    assert!(fs::read_to_string(repo.path().join("CHANGELOG.md"))?.contains("0.2.0"));
    assert_eq!(
        git_stdout(repo.path(), &["tag", "--list", "v0.2.0"])?.trim(),
        "v0.2.0"
    );
    assert_eq!(
        git_stdout(repo.path(), &["cat-file", "-t", "v0.2.0"])?.trim(),
        "tag"
    );
    assert!(git_stdout(repo.path(), &["log", "-1", "--pretty=%s"])?
        .contains("chore(release): release v0.2.0"));
    assert_eq!(
        git_stdout(repo.path(), &["status", "--porcelain"])?,
        String::new()
    );

    Ok(())
}

#[test]
fn release_prompts_before_each_mutating_step() -> Result<(), Box<dyn std::error::Error>> {
    let repo = TempDir::new()?;
    write_release_fixture(repo.path())?;

    Command::cargo_bin("verso")?
        .current_dir(repo.path())
        .args(["--version", "0.2.0"])
        .write_stdin("y\ny\ny\ny\n")
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "Modify release files for 0.2.0? [Y/n]",
        ))
        .stdout(predicate::str::contains(
            "Commit release files with \"chore(release): release v0.2.0\"? [Y/n]",
        ))
        .stdout(predicate::str::contains("Create tag v0.2.0? [Y/n]"))
        .stdout(predicate::str::contains(
            "Push release commit and tag? [Y/n]",
        ))
        .stderr(predicate::str::contains("git push --follow-tags"));

    Ok(())
}

#[test]
fn release_confirmation_defaults_to_yes() -> Result<(), Box<dyn std::error::Error>> {
    let repo = TempDir::new()?;
    write_release_fixture(repo.path())?;

    Command::cargo_bin("verso")?
        .current_dir(repo.path())
        .args(["--version", "0.2.0"])
        .write_stdin("\n\n\n\n")
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "Modify release files for 0.2.0? [Y/n]",
        ))
        .stdout(predicate::str::contains(
            "Commit release files with \"chore(release): release v0.2.0\"? [Y/n]",
        ))
        .stdout(predicate::str::contains("Create tag v0.2.0? [Y/n]"))
        .stdout(predicate::str::contains(
            "Push release commit and tag? [Y/n]",
        ))
        .stderr(predicate::str::contains("git push --follow-tags"));

    assert!(
        fs::read_to_string(repo.path().join("package.json"))?.contains("\"version\": \"0.2.0\"")
    );
    assert_eq!(
        git_stdout(repo.path(), &["tag", "--list", "v0.2.0"])?.trim(),
        "v0.2.0"
    );

    Ok(())
}

#[test]
fn abort_before_modifying_release_files_leaves_worktree_clean(
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = TempDir::new()?;
    write_release_fixture(repo.path())?;

    Command::cargo_bin("verso")?
        .current_dir(repo.path())
        .args(["--version", "0.2.0"])
        .write_stdin("n\n")
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "Modify release files for 0.2.0? [Y/n]",
        ))
        .stderr(predicate::str::contains("release aborted"));

    assert!(
        fs::read_to_string(repo.path().join("package.json"))?.contains("\"version\": \"0.1.0\"")
    );
    assert!(!fs::read_to_string(repo.path().join("CHANGELOG.md"))?.contains("0.2.0"));
    assert_eq!(
        git_stdout(repo.path(), &["status", "--porcelain"])?,
        String::new()
    );
    assert_eq!(
        git_stdout(repo.path(), &["tag", "--list", "v0.2.0"])?,
        String::new()
    );

    Ok(())
}

#[test]
fn abort_before_commit_keeps_release_files() -> Result<(), Box<dyn std::error::Error>> {
    let repo = TempDir::new()?;
    write_release_fixture(repo.path())?;
    let before_head = git_stdout(repo.path(), &["rev-parse", "HEAD"])?;

    Command::cargo_bin("verso")?
        .current_dir(repo.path())
        .args(["--version", "0.2.0"])
        .write_stdin("y\nn\n")
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "Commit release files with \"chore(release): release v0.2.0\"? [Y/n]",
        ))
        .stderr(predicate::str::contains("release aborted"));

    assert!(
        fs::read_to_string(repo.path().join("package.json"))?.contains("\"version\": \"0.2.0\"")
    );
    assert!(fs::read_to_string(repo.path().join("CHANGELOG.md"))?.contains("0.2.0"));
    assert_eq!(
        git_stdout(repo.path(), &["rev-parse", "HEAD"])?,
        before_head
    );
    let status = git_stdout(repo.path(), &["status", "--porcelain"])?;
    assert!(status.contains(" M CHANGELOG.md"));
    assert!(status.contains(" M package.json"));

    Ok(())
}

#[test]
fn abort_before_tag_keeps_release_commit_and_files() -> Result<(), Box<dyn std::error::Error>> {
    let repo = TempDir::new()?;
    write_release_fixture(repo.path())?;

    Command::cargo_bin("verso")?
        .current_dir(repo.path())
        .args(["--version", "0.2.0"])
        .write_stdin("y\ny\nn\n")
        .assert()
        .failure()
        .stdout(predicate::str::contains("Create tag v0.2.0? [Y/n]"))
        .stderr(predicate::str::contains("release aborted"));

    assert!(
        fs::read_to_string(repo.path().join("package.json"))?.contains("\"version\": \"0.2.0\"")
    );
    assert!(fs::read_to_string(repo.path().join("CHANGELOG.md"))?.contains("0.2.0"));
    assert!(git_stdout(repo.path(), &["log", "-1", "--pretty=%s"])?
        .contains("chore(release): release v0.2.0"));
    assert_eq!(
        git_stdout(repo.path(), &["status", "--porcelain"])?,
        String::new()
    );
    assert_eq!(
        git_stdout(repo.path(), &["tag", "--list", "v0.2.0"])?,
        String::new()
    );

    Ok(())
}

#[test]
fn abort_before_push_keeps_local_release_commit_and_tag() -> Result<(), Box<dyn std::error::Error>>
{
    let repo = TempDir::new()?;
    write_release_fixture(repo.path())?;

    Command::cargo_bin("verso")?
        .current_dir(repo.path())
        .args(["--version", "0.2.0"])
        .write_stdin("y\ny\ny\nn\n")
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "Push release commit and tag? [Y/n]",
        ))
        .stderr(predicate::str::contains("release aborted"));

    assert!(
        fs::read_to_string(repo.path().join("package.json"))?.contains("\"version\": \"0.2.0\"")
    );
    assert!(fs::read_to_string(repo.path().join("CHANGELOG.md"))?.contains("0.2.0"));
    assert!(git_stdout(repo.path(), &["log", "-1", "--pretty=%s"])?
        .contains("chore(release): release v0.2.0"));
    assert_eq!(
        git_stdout(repo.path(), &["tag", "--list", "v0.2.0"])?.trim(),
        "v0.2.0"
    );
    assert_eq!(
        git_stdout(repo.path(), &["status", "--porcelain"])?,
        String::new()
    );

    Ok(())
}

#[test]
fn existing_tag_blocks_release_without_writing_files() -> Result<(), Box<dyn std::error::Error>> {
    let repo = TempDir::new()?;
    write_release_fixture(repo.path())?;
    git(repo.path(), &["tag", "v0.2.0"])?;

    Command::cargo_bin("verso")?
        .current_dir(repo.path())
        .args(["--version", "0.2.0", "--yes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("tag v0.2.0 already exists"));

    assert!(
        fs::read_to_string(repo.path().join("package.json"))?.contains("\"version\": \"0.1.0\"")
    );
    assert!(!fs::read_to_string(repo.path().join("CHANGELOG.md"))?.contains("0.2.0"));
    assert_eq!(
        git_stdout(repo.path(), &["status", "--porcelain"])?,
        String::new()
    );

    Ok(())
}

#[test]
fn commit_failure_unstages_and_rolls_back_release_files() -> Result<(), Box<dyn std::error::Error>>
{
    let repo = TempDir::new()?;
    write_release_fixture(repo.path())?;
    git(repo.path(), &["config", "--unset", "user.email"])?;
    git(repo.path(), &["config", "--unset", "user.name"])?;
    let isolated_home = TempDir::new()?;
    let before_head = git_stdout(repo.path(), &["rev-parse", "HEAD"])?;

    Command::cargo_bin("verso")?
        .current_dir(repo.path())
        .env("HOME", isolated_home.path())
        .env("XDG_CONFIG_HOME", isolated_home.path())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "")
        .env("GIT_AUTHOR_EMAIL", "")
        .env("GIT_COMMITTER_NAME", "")
        .env("GIT_COMMITTER_EMAIL", "")
        .args(["--version", "0.2.0", "--yes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("git commit"));

    assert!(
        fs::read_to_string(repo.path().join("package.json"))?.contains("\"version\": \"0.1.0\"")
    );
    assert!(!fs::read_to_string(repo.path().join("CHANGELOG.md"))?.contains("0.2.0"));
    assert_eq!(
        git_stdout(repo.path(), &["rev-parse", "HEAD"])?,
        before_head
    );
    assert_eq!(
        git_stdout(repo.path(), &["status", "--porcelain"])?,
        String::new()
    );
    assert_eq!(
        git_stdout(repo.path(), &["tag", "--list", "v0.2.0"])?,
        String::new()
    );

    Ok(())
}

#[test]
fn add_failure_unstages_and_rolls_back_release_files() -> Result<(), Box<dyn std::error::Error>> {
    let repo = TempDir::new()?;
    write_release_fixture(repo.path())?;
    write_file(
        &repo.path().join("verso.toml"),
        r#"
[workspaces]
patterns = ["packages/*"]
include_root = true

[changelog]
infile = "ignored/CHANGELOG.md"
"#,
    )?;
    write_file(&repo.path().join(".gitignore"), "ignored/\n")?;
    git(repo.path(), &["add", "verso.toml", ".gitignore"])?;
    git(
        repo.path(),
        &["commit", "-m", "test: ignored changelog path"],
    )?;

    Command::cargo_bin("verso")?
        .current_dir(repo.path())
        .args(["--version", "0.2.0", "--yes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("git add"))
        .stderr(predicate::str::contains("ignored"));

    assert!(
        fs::read_to_string(repo.path().join("package.json"))?.contains("\"version\": \"0.1.0\"")
    );
    assert!(
        fs::read_to_string(repo.path().join("packages/a/package.json"))?
            .contains("\"version\": \"0.1.0\"")
    );
    assert!(!repo.path().join("ignored/CHANGELOG.md").exists());
    assert_eq!(
        git_stdout(repo.path(), &["status", "--porcelain"])?,
        String::new()
    );

    Ok(())
}

#[test]
fn explicit_non_forward_version_requires_confirmation() -> Result<(), Box<dyn std::error::Error>> {
    let repo = TempDir::new()?;
    write_release_fixture(repo.path())?;

    Command::cargo_bin("verso")?
        .current_dir(repo.path())
        .args(["--dry-run", "--version", "0.1.0"])
        .write_stdin("n\n")
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "Target version is not greater than current version. Continue? [Y/n]",
        ))
        .stderr(predicate::str::contains("release aborted"));

    Command::cargo_bin("verso")?
        .current_dir(repo.path())
        .args(["--dry-run", "--version", "0.0.9"])
        .write_stdin("n\n")
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "Target version is not greater than current version. Continue? [Y/n]",
        ))
        .stderr(predicate::str::contains("release aborted"));

    Command::cargo_bin("verso")?
        .current_dir(repo.path())
        .args(["--dry-run", "--version", "0.1.0"])
        .write_stdin("\n")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Target version is not greater than current version. Continue? [Y/n]",
        ))
        .stdout(predicate::str::contains("Target version: 0.1.0"));

    Command::cargo_bin("verso")?
        .current_dir(repo.path())
        .args(["--dry-run", "--version", "0.1.0", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Target version: 0.1.0"));

    Ok(())
}

#[test]
fn interactive_custom_equal_version_requires_confirmation() -> Result<(), Box<dyn std::error::Error>>
{
    let repo = TempDir::new()?;
    write_release_fixture(repo.path())?;

    Command::cargo_bin("verso")?
        .current_dir(repo.path())
        .args(["--dry-run"])
        .write_stdin("custom\n0.1.0\nn\n")
        .assert()
        .failure()
        .stdout(predicate::str::contains("Target version is not greater"))
        .stderr(predicate::str::contains("release aborted"));

    Ok(())
}

#[test]
fn interactive_beta_minor_dry_run_uses_computed_prerelease(
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = TempDir::new()?;
    write_release_fixture(repo.path())?;

    Command::cargo_bin("verso")?
        .current_dir(repo.path())
        .args(["--dry-run"])
        .write_stdin("beta\nminor\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Target version: 0.2.0-beta.0"));

    Ok(())
}

fn init_repo(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    git(path, &["init"])?;
    git(path, &["config", "user.email", "test@example.com"])?;
    git(path, &["config", "user.name", "Test User"])?;
    Ok(())
}

fn write_file(path: &Path, contents: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;
    Ok(())
}

fn write_release_fixture(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    init_repo(path)?;
    write_file(
        &path.join("package.json"),
        "{\n  \"name\": \"root\",\n  \"version\": \"0.1.0\"\n}\n",
    )?;
    write_file(
        &path.join("packages/a/package.json"),
        "{\n  \"name\": \"a\",\n  \"version\": \"0.1.0\"\n}\n",
    )?;
    write_file(
        &path.join("verso.toml"),
        r#"
[workspaces]
patterns = ["packages/*"]
include_root = true
"#,
    )?;
    write_file(&path.join("CHANGELOG.md"), "# Changelog\n")?;

    git(path, &["add", "."])?;
    git(path, &["commit", "-m", "chore: initial release"])?;
    git(path, &["tag", "v0.1.0"])?;
    write_file(&path.join("feature.md"), "feature\n")?;
    git(path, &["add", "feature.md"])?;
    git(path, &["commit", "-m", "feat: add feature (#1)"])?;

    Ok(())
}

fn git(path: &Path, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let output = ProcessCommand::new("git")
        .args(args)
        .current_dir(path)
        .output()?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        )
        .into())
    }
}

fn git_stdout(path: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let output = ProcessCommand::new("git")
        .args(args)
        .current_dir(path)
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        )
        .into())
    }
}
