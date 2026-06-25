use semver::Version;
use std::{ops::Range, path::Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageVersion {
    pub name: String,
    pub version: Version,
}

pub fn replace_package_version_preserving_style(
    path: &Path,
    contents: &str,
    next: &Version,
) -> Result<String, String> {
    read_package_version(path, contents)?;
    let range = find_package_version_value_range(contents)?.ok_or_else(|| {
        format!(
            "failed to locate [package].version string in {}",
            path.display()
        )
    })?;

    let mut updated = String::with_capacity(contents.len());
    updated.push_str(&contents[..range.start]);
    updated.push_str(&next.to_string());
    updated.push_str(&contents[range.end..]);
    Ok(updated)
}

pub fn read_package_version(path: &Path, contents: &str) -> Result<PackageVersion, String> {
    let value = toml::from_str::<toml::Value>(contents)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    let package = value
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{} is missing [package]", path.display()))?;
    let name = package
        .get("name")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} is missing [package].name string", path.display()))?;
    let version = package
        .get("version")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} is missing [package].version string", path.display()))?;

    let version = Version::parse(version).map_err(|error| {
        format!(
            "{} has invalid semver package version \"{version}\": {error}",
            path.display()
        )
    })?;
    Ok(PackageVersion {
        name: name.to_string(),
        version,
    })
}

pub fn replace_lock_package_version_preserving_style(
    path: &Path,
    contents: &str,
    package_name: &str,
    current: &Version,
    next: &Version,
) -> Result<String, String> {
    validate_lock_package_version(path, contents, package_name, current)?;
    let range = find_lock_package_version_value_range(contents, package_name, current)?
        .ok_or_else(|| {
            format!(
                "failed to locate Cargo.lock package {package_name} version {current} in {}",
                path.display()
            )
        })?;

    let mut updated = String::with_capacity(contents.len());
    updated.push_str(&contents[..range.start]);
    updated.push_str(&next.to_string());
    updated.push_str(&contents[range.end..]);
    Ok(updated)
}

fn validate_lock_package_version(
    path: &Path,
    contents: &str,
    package_name: &str,
    current: &Version,
) -> Result<(), String> {
    let value = toml::from_str::<toml::Value>(contents)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    let packages = value
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{} is missing [[package]] entries", path.display()))?;
    let matches = packages
        .iter()
        .filter(|package| {
            package
                .get("name")
                .and_then(toml::Value::as_str)
                .is_some_and(|name| name == package_name)
                && package
                    .get("version")
                    .and_then(toml::Value::as_str)
                    .is_some_and(|version| version == current.to_string())
        })
        .count();

    match matches {
        0 => Err(format!(
            "{} is missing Cargo.lock package {package_name} version {current}",
            path.display()
        )),
        1 => Ok(()),
        _ => Err(format!(
            "{} contains multiple Cargo.lock packages named {package_name} at version {current}",
            path.display()
        )),
    }
}

fn find_package_version_value_range(contents: &str) -> Result<Option<Range<usize>>, String> {
    let mut in_package = false;
    let mut version_range = None;
    let mut offset = 0;

    for line in contents.split_inclusive('\n') {
        if let Some(table) = table_name(line) {
            in_package = table == "package";
        } else if in_package {
            if let Some((range, _value)) = string_value_range(line, "version", offset)? {
                if version_range.is_some() {
                    return Err("duplicate [package].version keys are not supported".to_owned());
                }
                version_range = Some(range);
            }
        }

        offset += line.len();
    }

    Ok(version_range)
}

fn find_lock_package_version_value_range(
    contents: &str,
    package_name: &str,
    current: &Version,
) -> Result<Option<Range<usize>>, String> {
    let mut in_package = false;
    let mut entry_name: Option<String> = None;
    let mut entry_version: Option<(String, Range<usize>)> = None;
    let mut matched_range = None;
    let mut offset = 0;

    for line in contents.split_inclusive('\n') {
        if let Some(table) = table_name(line) {
            if in_package {
                maybe_capture_lock_match(
                    package_name,
                    current,
                    &entry_name,
                    &entry_version,
                    &mut matched_range,
                )?;
            }

            in_package = table == "package";
            entry_name = None;
            entry_version = None;
        } else if in_package {
            if let Some((_range, value)) = string_value_range(line, "name", offset)? {
                entry_name = Some(value);
            }
            if let Some((range, value)) = string_value_range(line, "version", offset)? {
                entry_version = Some((value, range));
            }
        }

        offset += line.len();
    }

    if in_package {
        maybe_capture_lock_match(
            package_name,
            current,
            &entry_name,
            &entry_version,
            &mut matched_range,
        )?;
    }

    Ok(matched_range)
}

fn maybe_capture_lock_match(
    package_name: &str,
    current: &Version,
    entry_name: &Option<String>,
    entry_version: &Option<(String, Range<usize>)>,
    matched_range: &mut Option<Range<usize>>,
) -> Result<(), String> {
    let Some(name) = entry_name else {
        return Ok(());
    };
    let Some((version, range)) = entry_version else {
        return Ok(());
    };

    if name == package_name && version == &current.to_string() {
        if matched_range.is_some() {
            return Err(format!(
                "duplicate Cargo.lock package {package_name} version {current} entries are not supported"
            ));
        }
        *matched_range = Some(range.clone());
    }

    Ok(())
}

fn table_name(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("[[") {
        return trimmed
            .find("]]")
            .map(|end| trimmed[2..end].trim())
            .filter(|name| !name.is_empty());
    }

    if !trimmed.starts_with('[') {
        return None;
    }

    trimmed
        .find(']')
        .map(|end| trimmed[1..end].trim())
        .filter(|name| !name.is_empty())
}

fn string_value_range(
    line: &str,
    key: &str,
    line_offset: usize,
) -> Result<Option<(Range<usize>, String)>, String> {
    let leading = line.len() - line.trim_start().len();
    let rest = &line[leading..];
    let Some(after_key) = rest.strip_prefix(key) else {
        return Ok(None);
    };

    let skipped_after_key = after_key.len() - after_key.trim_start().len();
    let after_key = after_key.trim_start();
    if !after_key.starts_with('=') {
        return Ok(None);
    }

    let equals = leading + key.len() + skipped_after_key;
    let after_equals = equals + 1;
    let skipped_after_equals = line[after_equals..].len() - line[after_equals..].trim_start().len();
    let quote = after_equals + skipped_after_equals;

    let Some(delimiter) = line.as_bytes().get(quote).copied() else {
        return Err(format!("{key} must be a quoted string"));
    };
    if delimiter != b'"' && delimiter != b'\'' {
        return Err(format!("{key} must be a quoted string"));
    }

    let value_start = quote + 1;
    let mut escaped = false;
    for index in value_start..line.len() {
        let byte = line.as_bytes()[index];
        if delimiter == b'"' && !escaped && byte == b'\\' {
            escaped = true;
            continue;
        }
        if !escaped && byte == delimiter {
            return Ok(Some((
                (line_offset + value_start)..(line_offset + index),
                line[value_start..index].to_string(),
            )));
        }
        escaped = false;
    }

    Err(format!("{key} string is missing a closing quote"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_package_version_and_preserves_formatting() -> Result<(), String> {
        let contents = concat!(
            "[package]\n",
            "name = \"verso\"\n",
            "version = \"1.0.0-alpha.0\" # release version\n",
            "\n",
            "[dependencies]\n",
            "demo = { version = \"9.9.9\" }\n",
        );

        let updated = replace_package_version_preserving_style(
            Path::new("Cargo.toml"),
            contents,
            &Version::parse("1.0.0-alpha.1").map_err(|error| error.to_string())?,
        )?;

        assert!(updated.contains("version = \"1.0.0-alpha.1\" # release version"));
        assert!(updated.contains("demo = { version = \"9.9.9\" }"));
        Ok(())
    }

    #[test]
    fn does_not_replace_workspace_package_version() -> Result<(), String> {
        let contents = concat!(
            "[workspace.package]\n",
            "version = \"9.9.9\"\n",
            "\n",
            "[package]\n",
            "name = \"verso\"\n",
            "version = \"1.0.0\"\n",
        );

        let updated = replace_package_version_preserving_style(
            Path::new("Cargo.toml"),
            contents,
            &Version::parse("1.1.0").map_err(|error| error.to_string())?,
        )?;

        assert!(updated.contains("[workspace.package]\nversion = \"9.9.9\""));
        assert!(updated.contains("[package]\nname = \"verso\"\nversion = \"1.1.0\""));
        Ok(())
    }

    #[test]
    fn rejects_missing_package_version_string() {
        let error = replace_package_version_preserving_style(
            Path::new("Cargo.toml"),
            "[package]\nname = \"verso\"\nversion.workspace = true\n",
            &Version::new(1, 1, 0),
        )
        .expect_err("workspace-inherited package version should be rejected");

        assert!(error.contains("[package].version string"));
    }

    #[test]
    fn replaces_matching_cargo_lock_package_version_only() -> Result<(), String> {
        let contents = concat!(
            "# This file is automatically @generated by Cargo.\n",
            "version = 4\n",
            "\n",
            "[[package]]\n",
            "name = \"demo\"\n",
            "version = \"0.1.0\"\n",
            "\n",
            "[[package]]\n",
            "name = \"verso\"\n",
            "version = \"1.0.0-alpha.0\"\n",
            "dependencies = [\"demo\"]\n",
        );

        let updated = replace_lock_package_version_preserving_style(
            Path::new("Cargo.lock"),
            contents,
            "verso",
            &Version::parse("1.0.0-alpha.0").map_err(|error| error.to_string())?,
            &Version::parse("1.0.0-alpha.1").map_err(|error| error.to_string())?,
        )?;

        assert!(updated.contains("name = \"demo\"\nversion = \"0.1.0\""));
        assert!(updated.contains("name = \"verso\"\nversion = \"1.0.0-alpha.1\""));
        assert!(updated.contains("dependencies = [\"demo\"]"));
        Ok(())
    }
}
