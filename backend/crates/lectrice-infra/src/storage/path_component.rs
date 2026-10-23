use lectrice_domain::{AppError, AppResult, ErrorName};

/// Ids that become file or directory names (engine, voice, export ids) must be plain names, never paths.
pub(crate) fn safe_component(value: &str) -> AppResult<&str> {
    let is_plain = !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        && value.bytes().any(|byte| byte != b'.');
    if is_plain {
        return Ok(value);
    }
    Err(AppError::new(ErrorName::StorageFailed).with_detail(format!("unsafe path component {value:?}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_names_pass() {
        for good in ["af_heart", "kokoro-v1.0", "a.b"] {
            assert!(safe_component(good).is_ok(), "{good}");
        }
        for bad in ["", ".", "..", "a/b", "../x", "a b", "a\\b"] {
            assert!(safe_component(bad).is_err(), "{bad}");
        }
    }
}
