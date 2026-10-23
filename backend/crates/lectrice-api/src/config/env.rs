use toml::{Table, Value};

use super::ConfigError;

/// Names the config file itself, so it is not an override.
pub const CONFIG_PATH_ENV: &str = "LECTRICE_CONFIG";

const OVERRIDE_PREFIX: &str = "LECTRICE_";
const SECTION_SEPARATOR: char = '_';

/// `LECTRICE_AUDIO_CACHE_MAX_MB=512` sets `[audio] cache_max_mb`, parsed as the type the file uses.
pub fn apply_overrides(table: &mut Table, env: impl IntoIterator<Item = (String, String)>) -> Result<(), ConfigError> {
    for (name, raw) in env {
        let Some(path) = name.strip_prefix(OVERRIDE_PREFIX) else { continue };
        if name == CONFIG_PATH_ENV {
            continue;
        }
        let slot = find_slot(table, &path.to_ascii_lowercase()).ok_or_else(|| ConfigError::UnknownOverride(name.clone()))?;
        *slot = parse_like(slot, &raw).ok_or(ConfigError::BadOverride(name))?;
    }
    Ok(())
}

fn find_slot<'a>(table: &'a mut Table, path: &str) -> Option<&'a mut Value> {
    let (section, key) = path.split_once(SECTION_SEPARATOR)?;
    table.get_mut(section)?.as_table_mut()?.get_mut(key)
}

fn parse_like(current: &Value, raw: &str) -> Option<Value> {
    match current {
        Value::String(_) => Some(Value::String(raw.to_string())),
        Value::Integer(_) => raw.trim().parse().ok().map(Value::Integer),
        Value::Float(_) => raw.trim().parse().ok().map(Value::Float),
        Value::Boolean(_) => raw.trim().parse().ok().map(Value::Boolean),
        _ => None,
    }
}
