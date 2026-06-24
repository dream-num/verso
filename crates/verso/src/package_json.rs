use semver::Version;
use serde_json::Value;
use std::fs;
use std::ops::Range;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageInfo {
    pub name: Option<String>,
    pub version: Version,
}

pub fn read_package(path: &Path) -> Result<PackageInfo, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

    read_package_from_str(path, &contents)
}

pub fn read_package_from_str(path: &Path, contents: &str) -> Result<PackageInfo, String> {
    let value: Value = serde_json::from_str(contents)
        .map_err(|error| format!("failed to parse {} as JSON: {error}", path.display()))?;

    package_info_from_value(path, &value)
}

pub fn replace_version_preserving_style(contents: &str, next: &Version) -> Result<String, String> {
    let value: Value = serde_json::from_str(contents)
        .map_err(|error| format!("failed to parse package JSON contents: {error}"))?;
    package_info_from_value(Path::new("package.json"), &value)?;

    let range = find_top_level_version_value_range(contents)?
        .ok_or_else(|| "failed to locate top-level package version string".to_owned())?;

    let mut updated = String::with_capacity(contents.len() + next.to_string().len());
    updated.push_str(&contents[..range.start]);
    updated.push_str(&next.to_string());
    updated.push_str(&contents[range.end..]);
    Ok(updated)
}

fn package_info_from_value(path: &Path, value: &Value) -> Result<PackageInfo, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{} must contain a JSON object", path.display()))?;
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let context = package_context(path, name.as_deref());
    let version_value = object
        .get("version")
        .ok_or_else(|| format!("{context} is missing required string field \"version\""))?;
    let version_string = version_value
        .as_str()
        .ok_or_else(|| format!("{context} field \"version\" must be a string"))?;
    let version = Version::parse(version_string).map_err(|error| {
        format!("{context} has invalid semver version \"{version_string}\": {error}")
    })?;

    Ok(PackageInfo { name, version })
}

fn package_context(path: &Path, name: Option<&str>) -> String {
    match name {
        Some(name) => format!("package {name} ({})", path.display()),
        None => format!("package {}", path.display()),
    }
}

fn find_top_level_version_value_range(contents: &str) -> Result<Option<Range<usize>>, String> {
    let bytes = contents.as_bytes();
    let mut cursor = skip_whitespace(bytes, 0);
    let mut version_range = None;

    if bytes.get(cursor) != Some(&b'{') {
        return Ok(None);
    }
    cursor += 1;

    loop {
        cursor = skip_whitespace(bytes, cursor);
        match bytes.get(cursor) {
            Some(b'}') => return Ok(version_range),
            Some(b'"') => {}
            _ => return Ok(None),
        }

        let Some(key_end) = find_string_end(bytes, cursor) else {
            return Ok(None);
        };
        let Ok(key) = serde_json::from_str::<String>(&contents[cursor..=key_end]) else {
            return Ok(None);
        };
        cursor = skip_whitespace(bytes, key_end + 1);
        if bytes.get(cursor) != Some(&b':') {
            return Ok(None);
        }
        cursor = skip_whitespace(bytes, cursor + 1);

        if key == "version" {
            if bytes.get(cursor) != Some(&b'"') {
                return Ok(None);
            }
            let Some(value_end) = find_string_end(bytes, cursor) else {
                return Ok(None);
            };
            if version_range.is_some() {
                return Err("duplicate top-level version keys are not supported".to_owned());
            }
            version_range = Some((cursor + 1)..value_end);
            cursor = value_end + 1;
        } else {
            let Some(next_cursor) = skip_json_value(bytes, cursor) else {
                return Ok(None);
            };
            cursor = next_cursor;
        }

        cursor = skip_whitespace(bytes, cursor);
        match bytes.get(cursor) {
            Some(b',') => cursor += 1,
            Some(b'}') => return Ok(version_range),
            _ => return Ok(None),
        }
    }
}

fn skip_whitespace(bytes: &[u8], mut cursor: usize) -> usize {
    while matches!(bytes.get(cursor), Some(b' ' | b'\n' | b'\r' | b'\t')) {
        cursor += 1;
    }
    cursor
}

fn find_string_end(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'"') {
        return None;
    }

    let mut cursor = start + 1;
    while let Some(byte) = bytes.get(cursor) {
        match byte {
            b'\\' => cursor += 2,
            b'"' => return Some(cursor),
            _ => cursor += 1,
        }
    }
    None
}

fn skip_json_value(bytes: &[u8], start: usize) -> Option<usize> {
    match bytes.get(start)? {
        b'"' => find_string_end(bytes, start).map(|end| end + 1),
        b'{' | b'[' => skip_json_container(bytes, start),
        _ => skip_json_scalar(bytes, start),
    }
}

fn skip_json_container(bytes: &[u8], start: usize) -> Option<usize> {
    let mut stack = vec![matching_close(*bytes.get(start)?)?];
    let mut cursor = start + 1;
    while let Some(byte) = bytes.get(cursor) {
        match byte {
            b'"' => cursor = find_string_end(bytes, cursor)? + 1,
            b'{' | b'[' => {
                stack.push(matching_close(*byte)?);
                cursor += 1;
            }
            b'}' | b']' => {
                if stack.pop() != Some(*byte) {
                    return None;
                }
                cursor += 1;
                if stack.is_empty() {
                    return Some(cursor);
                }
            }
            _ => cursor += 1,
        }
    }

    None
}

fn matching_close(open: u8) -> Option<u8> {
    match open {
        b'{' => Some(b'}'),
        b'[' => Some(b']'),
        _ => None,
    }
}

fn skip_json_scalar(bytes: &[u8], start: usize) -> Option<usize> {
    let mut cursor = start;
    while let Some(byte) = bytes.get(cursor) {
        match byte {
            b',' | b'}' | b']' | b' ' | b'\n' | b'\r' | b'\t' => break,
            _ => cursor += 1,
        }
    }

    (cursor > start).then_some(cursor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn reads_package_version_and_optional_name() -> Result<(), String> {
        let path = Path::new("packages/widget/package.json");
        let package = read_package_from_str(
            path,
            r#"{
  "name": "@scope/widget",
  "version": "1.2.3-beta.4"
}"#,
        )?;

        assert_eq!(package.name, Some("@scope/widget".to_owned()));
        assert_eq!(
            package.version,
            Version::parse("1.2.3-beta.4").expect("test semver should parse")
        );
        Ok(())
    }

    #[test]
    fn reads_package_without_name() -> Result<(), String> {
        let package = read_package_from_str(
            Path::new("package.json"),
            r#"{
  "version": "0.1.0"
}"#,
        )?;

        assert_eq!(package.name, None);
        assert_eq!(package.version, Version::new(0, 1, 0));
        Ok(())
    }

    #[test]
    fn preserves_formatting_around_version_line() -> Result<(), String> {
        let contents =
            "{\n    \"name\": \"demo\",\n    \"version\": \"1.0.0\",\n    \"private\": true\n}\n";
        let next = Version::new(1, 1, 0);

        let updated = replace_version_preserving_style(contents, &next)?;

        assert_eq!(
            updated,
            "{\n    \"name\": \"demo\",\n    \"version\": \"1.1.0\",\n    \"private\": true\n}\n"
        );
        Ok(())
    }

    #[test]
    fn updates_compact_json() -> Result<(), String> {
        let updated = replace_version_preserving_style(
            r#"{"name":"demo","version":"1.0.0"}"#,
            &Version::new(1, 2, 3),
        )?;

        assert_eq!(updated, r#"{"name":"demo","version":"1.2.3"}"#);
        Ok(())
    }

    #[test]
    fn updates_reordered_top_level_fields() -> Result<(), String> {
        let contents =
            "{\n  \"private\": true,\n  \"name\": \"demo\",\n  \"version\": \"1.0.0\"\n}";

        let updated = replace_version_preserving_style(contents, &Version::new(1, 2, 3))?;

        assert_eq!(
            updated,
            "{\n  \"private\": true,\n  \"name\": \"demo\",\n  \"version\": \"1.2.3\"\n}"
        );
        Ok(())
    }

    #[test]
    fn rejects_duplicate_top_level_version_keys() {
        let error = replace_version_preserving_style(
            r#"{"version":"1.0.0","version":"2.0.0"}"#,
            &Version::new(3, 0, 0),
        )
        .expect_err("duplicate top-level version keys should be rejected");

        assert!(error.contains("duplicate top-level version"));
    }

    #[test]
    fn rejects_missing_version() {
        let error = read_package_from_str(Path::new("package.json"), r#"{"name":"demo"}"#)
            .expect_err("missing version should be rejected");

        assert!(error.contains("package.json"));
        assert!(error.contains("version"));
    }

    #[test]
    fn rejects_invalid_json() {
        let error =
            read_package_from_str(Path::new("broken/package.json"), r#"{"version":"1.0.0""#)
                .expect_err("invalid JSON should be rejected");

        assert!(error.contains("broken/package.json"));
        assert!(error.contains("JSON"));
    }

    #[test]
    fn rejects_non_string_version() {
        let error =
            read_package_from_str(Path::new("packages/demo/package.json"), r#"{"version":1}"#)
                .expect_err("non-string version should be rejected");

        assert!(error.contains("packages/demo/package.json"));
        assert!(error.contains("version"));
        assert!(error.contains("string"));
    }

    #[test]
    fn rejects_invalid_semver() {
        let error = read_package_from_str(
            Path::new("package.json"),
            r#"{"name":"demo","version":"1.2"}"#,
        )
        .expect_err("invalid semver should be rejected");

        assert!(error.contains("demo"));
        assert!(error.contains("1.2"));
        assert!(error.contains("semver"));
    }

    #[test]
    fn preserves_crlf_and_final_newline_style() -> Result<(), String> {
        let contents = "{\r\n  \"version\": \"1.0.0\"\r\n}\r\n";

        let updated = replace_version_preserving_style(contents, &Version::new(2, 0, 0))?;

        assert_eq!(updated, "{\r\n  \"version\": \"2.0.0\"\r\n}\r\n");
        Ok(())
    }

    #[test]
    fn does_not_replace_nested_dependency_versions() -> Result<(), String> {
        let contents = "{\n  \"dependencies\": {\n    \"demo\": {\n      \"version\": \"9.9.9\"\n    }\n  },\n  \"version\": \"1.0.0\"\n}\n";

        let updated = replace_version_preserving_style(contents, &Version::new(1, 2, 0))?;

        assert!(updated.contains("\"version\": \"1.2.0\""));
        assert!(updated.contains("\"version\": \"9.9.9\""));
        Ok(())
    }
}
