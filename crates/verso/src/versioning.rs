use semver::{Prerelease, Version};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseBump {
    Patch,
    Minor,
    Major,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrereleaseChannel {
    Alpha,
    Beta,
    Rc,
}

pub fn bump_stable(current: &Version, bump: BaseBump) -> Version {
    match bump {
        BaseBump::Patch => Version::new(current.major, current.minor, current.patch + 1),
        BaseBump::Minor => Version::new(current.major, current.minor + 1, 0),
        BaseBump::Major => Version::new(current.major + 1, 0, 0),
    }
}

pub fn bump_prerelease(
    current: &Version,
    base_bump: BaseBump,
    channel: PrereleaseChannel,
) -> Version {
    let mut next = prerelease_base(current, base_bump);
    let channel_name = channel.as_str();
    let next_number = prerelease_number(current, &next, channel_name).map_or(0, |current_number| {
        current_number.checked_add(1).unwrap_or(0)
    });

    next.pre = Prerelease::new(&format!("{channel_name}.{next_number}"))
        .expect("generated prerelease identifier should be valid semver");
    next
}

pub fn parse_custom_version(input: &str) -> Result<Version, String> {
    Version::parse(input).map_err(|error| format!("invalid semver version \"{input}\": {error}"))
}

impl PrereleaseChannel {
    fn as_str(self) -> &'static str {
        match self {
            PrereleaseChannel::Alpha => "alpha",
            PrereleaseChannel::Beta => "beta",
            PrereleaseChannel::Rc => "rc",
        }
    }
}

fn prerelease_base(current: &Version, base_bump: BaseBump) -> Version {
    // Product rule: minor continues the current prerelease train; patch/major start a new base.
    match (current.pre.is_empty(), base_bump) {
        (true, _) => bump_stable(current, base_bump),
        (false, BaseBump::Minor) => Version::new(current.major, current.minor, current.patch),
        (false, BaseBump::Patch | BaseBump::Major) => bump_stable(current, base_bump),
    }
}

fn prerelease_number(current: &Version, base: &Version, channel: &str) -> Option<u64> {
    if current.major != base.major || current.minor != base.minor || current.patch != base.patch {
        return None;
    }

    let mut identifiers = current.pre.as_str().split('.');
    let current_channel = identifiers.next()?;
    let current_number = identifiers.next()?;

    if identifiers.next().is_some() || current_channel != channel {
        return None;
    }

    current_number.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use semver::Version;

    #[test]
    fn stable_bumps_patch_minor_and_major() {
        let current = Version::new(0, 25, 0);

        assert_eq!(
            bump_stable(&current, BaseBump::Patch),
            Version::new(0, 25, 1)
        );
        assert_eq!(
            bump_stable(&current, BaseBump::Minor),
            Version::new(0, 26, 0)
        );
        assert_eq!(
            bump_stable(&current, BaseBump::Major),
            Version::new(1, 0, 0)
        );
    }

    #[test]
    fn prerelease_uses_selected_base_bump_and_channel() {
        let current = Version::new(0, 25, 0);

        assert_eq!(
            bump_prerelease(&current, BaseBump::Minor, PrereleaseChannel::Beta),
            Version::parse("0.26.0-beta.0").expect("test semver should parse")
        );
    }

    #[test]
    fn prerelease_same_base_and_channel_increments_number() {
        let current = Version::parse("0.26.0-beta.0").expect("test semver should parse");

        assert_eq!(
            bump_prerelease(&current, BaseBump::Minor, PrereleaseChannel::Beta),
            Version::parse("0.26.0-beta.1").expect("test semver should parse")
        );
    }

    #[test]
    fn prerelease_channel_switch_resets_number() {
        let current = Version::parse("0.26.0-beta.1").expect("test semver should parse");

        assert_eq!(
            bump_prerelease(&current, BaseBump::Minor, PrereleaseChannel::Rc),
            Version::parse("0.26.0-rc.0").expect("test semver should parse")
        );
    }

    #[test]
    fn prerelease_patch_and_major_honor_selected_bump_from_current_base() {
        let current = Version::parse("0.26.0-beta.0").expect("test semver should parse");

        assert_eq!(
            bump_prerelease(&current, BaseBump::Patch, PrereleaseChannel::Alpha),
            Version::parse("0.26.1-alpha.0").expect("test semver should parse")
        );
        assert_eq!(
            bump_prerelease(&current, BaseBump::Major, PrereleaseChannel::Alpha),
            Version::parse("1.0.0-alpha.0").expect("test semver should parse")
        );
    }

    #[test]
    fn stable_bumps_clear_build_metadata() {
        let current = Version::parse("1.2.3+build.7").expect("test semver should parse");

        assert_eq!(
            bump_stable(&current, BaseBump::Patch),
            Version::parse("1.2.4").expect("test semver should parse")
        );
    }

    #[test]
    fn prerelease_bumps_clear_build_metadata() {
        let current = Version::parse("1.2.3-beta.0+build.7").expect("test semver should parse");

        assert_eq!(
            bump_prerelease(&current, BaseBump::Minor, PrereleaseChannel::Beta),
            Version::parse("1.2.3-beta.1").expect("test semver should parse")
        );
    }

    #[test]
    fn prerelease_suffix_overflow_resets_to_zero() {
        let current =
            Version::parse("1.2.3-beta.18446744073709551615").expect("test semver should parse");

        assert_eq!(
            bump_prerelease(&current, BaseBump::Minor, PrereleaseChannel::Beta),
            Version::parse("1.2.3-beta.0").expect("test semver should parse")
        );
    }

    #[test]
    fn custom_version_parser_accepts_valid_semver() -> Result<(), String> {
        let version = parse_custom_version("1.2.3-alpha.9")?;

        assert_eq!(
            version,
            Version::parse("1.2.3-alpha.9").expect("test semver should parse")
        );
        Ok(())
    }

    #[test]
    fn custom_version_parser_rejects_invalid_semver() {
        let error = parse_custom_version("1.2").expect_err("invalid semver should be rejected");

        assert!(error.contains("invalid semver"));
        assert!(error.contains("1.2"));
    }
}
