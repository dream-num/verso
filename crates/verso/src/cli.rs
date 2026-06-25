use clap::Parser;

#[derive(Debug, Parser, PartialEq, Eq)]
#[command(
    name = "verso",
    version,
    disable_version_flag = true,
    about = "Release configured workspace packages with changelog, git tag, and push",
    long_about = "Verso is a focused release CLI. It reads verso.toml, updates package versions, generates an angular-style changelog, commits, tags, and pushes with git push --follow-tags. Use --dry-run to preview without changing files."
)]
pub struct Cli {
    #[arg(
        long,
        help = "Preview the release without writing files or running mutating git commands"
    )]
    pub dry_run: bool,

    #[arg(
        long = "version",
        value_name = "SEMVER",
        help = "Use a target version without interactive selection"
    )]
    pub target_version: Option<String>,

    #[arg(
        long,
        value_name = "PATH",
        default_value = "verso.toml",
        help = "Path to the Verso config file"
    )]
    pub config: String,

    #[arg(long, help = "Skip release confirmation prompts")]
    pub yes: bool,

    #[arg(
        short = 'V',
        long = "tool-version",
        help = "Print the Verso CLI version and exit"
    )]
    pub tool_version: bool,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn help_mentions_dry_run_and_version() {
        let mut command = Cli::command();
        let mut buffer = Vec::new();
        command
            .write_long_help(&mut buffer)
            .expect("long help should render");
        let help = String::from_utf8(buffer).expect("help should be valid UTF-8");
        assert!(help.contains("--dry-run"));
        assert!(help.contains("--version <SEMVER>"));
        assert!(help.contains("verso.toml"));
        assert!(help.contains("Skip release confirmation prompts"));
        assert!(help.contains("-V, --tool-version"));
    }

    #[test]
    fn parses_release_options() {
        let cli = Cli::try_parse_from([
            "verso",
            "--dry-run",
            "--version",
            "1.2.3",
            "--config",
            "custom.toml",
            "--yes",
        ])
        .expect("release options should parse");

        assert_eq!(
            cli,
            Cli {
                dry_run: true,
                target_version: Some("1.2.3".to_string()),
                config: "custom.toml".to_string(),
                yes: true,
                tool_version: false,
            }
        );
    }

    #[test]
    fn parses_tool_version_option() {
        let cli = Cli::try_parse_from(["verso", "--tool-version"])
            .expect("tool version option should parse");

        assert_eq!(
            cli,
            Cli {
                dry_run: false,
                target_version: None,
                config: "verso.toml".to_string(),
                yes: false,
                tool_version: true,
            }
        );
    }
}
