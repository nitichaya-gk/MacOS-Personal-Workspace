//! Database property types: value validation and type conversion.

use crate::error::{invalid, Result};
use serde_json::{json, Value};

pub const PROPERTY_TYPES: &[&str] = &[
    "title",
    "text",
    "number",
    "select",
    "multi_select",
    "status",
    "date",
    "checkbox",
    "url",
    "email",
    "files",
    "created_time",
    "last_edited_time",
    "relation",
    "formula",
    "rollup",
];

/// Types whose values are computed and never stored in record_values.
pub fn is_computed(t: &str) -> bool {
    matches!(t, "title" | "created_time" | "last_edited_time" | "formula" | "rollup")
}

pub fn options(config: &Value) -> Vec<Value> {
    config.get("options").and_then(|o| o.as_array()).cloned().unwrap_or_default()
}

pub fn option_name(config: &Value, id: &str) -> Option<String> {
    options(config)
        .into_iter()
        .find(|o| o.get("id").and_then(|v| v.as_str()) == Some(id))
        .and_then(|o| o.get("name").and_then(|n| n.as_str()).map(String::from))
}

fn option_exists(config: &Value, id: &str) -> bool {
    options(config).iter().any(|o| o.get("id").and_then(|v| v.as_str()) == Some(id))
}

pub fn is_valid_date(s: &str) -> bool {
    chrono::NaiveDate::parse_from_str(s.get(0..10).unwrap_or(""), "%Y-%m-%d").is_ok()
}

/// Validates a value for a property. `Ok(None)` means "clear the value".
pub fn validate_value(ptype: &str, config: &Value, value: &Value) -> Result<Option<Value>> {
    if value.is_null() {
        return Ok(None);
    }
    let v = match ptype {
        "text" | "url" => match value.as_str() {
            Some("") => return Ok(None),
            Some(s) => json!(s),
            None => return invalid("expected text"),
        },
        "email" => match value.as_str() {
            Some("") => return Ok(None),
            Some(s) if s.contains('@') && !s.contains(' ') => json!(s),
            _ => return invalid("expected an email address"),
        },
        "number" => match value {
            Value::Number(_) => value.clone(),
            Value::String(s) if s.trim().is_empty() => return Ok(None),
            Value::String(s) => match s.trim().parse::<f64>() {
                Ok(n) => json!(n),
                Err(_) => return invalid("expected a number"),
            },
            _ => return invalid("expected a number"),
        },
        "select" | "status" => match value.as_str() {
            Some(id) if option_exists(config, id) => json!(id),
            _ => return invalid("unknown option"),
        },
        "multi_select" => match value.as_array() {
            Some(ids) => {
                let mut out = vec![];
                for id in ids {
                    match id.as_str() {
                        Some(id) if option_exists(config, id) => out.push(json!(id)),
                        _ => return invalid("unknown option"),
                    }
                }
                if out.is_empty() {
                    return Ok(None);
                }
                Value::Array(out)
            }
            None => return invalid("expected a list of options"),
        },
        "date" => {
            let start = value.get("start").and_then(|s| s.as_str()).unwrap_or("");
            if !is_valid_date(start) {
                return invalid("expected a date (YYYY-MM-DD)");
            }
            let mut d = json!({ "start": start });
            if let Some(end) = value.get("end").and_then(|e| e.as_str()) {
                if !is_valid_date(end) || end < start {
                    return invalid("end date must be a date after start");
                }
                d["end"] = json!(end);
            }
            d
        }
        "checkbox" => match value.as_bool() {
            Some(true) => json!(true),
            Some(false) => return Ok(None),
            None => return invalid("expected true or false"),
        },
        "relation" => match value.as_array() {
            Some(ids) if ids.iter().all(|i| i.as_str().is_some_and(crate::domain::is_uuid)) => {
                if ids.is_empty() {
                    return Ok(None);
                }
                let mut uniq: Vec<Value> = vec![];
                for i in ids {
                    if !uniq.contains(i) {
                        uniq.push(i.clone());
                    }
                }
                Value::Array(uniq)
            }
            _ => return invalid("expected a list of record ids"),
        },
        "files" => match value.as_array() {
            Some(files) if files.iter().all(|f| f.get("assetId").and_then(|a| a.as_str()).is_some()) => {
                if files.is_empty() {
                    return Ok(None);
                }
                value.clone()
            }
            _ => return invalid("expected a list of files"),
        },
        other => return invalid(format!("property type {other} has no stored value")),
    };
    Ok(Some(v))
}

/// Human-readable text of a value (CSV export, search, conversion to text).
pub fn display_value(ptype: &str, config: &Value, value: &Value) -> String {
    if let Some(err) = value.get("error").and_then(|e| e.as_str()) {
        return format!("#ERROR: {err}");
    }
    match ptype {
        "formula" | "rollup" => match value {
            Value::Object(o) if o.contains_key("start") => o["start"].as_str().unwrap_or("").to_string(),
            Value::Array(a) => a.iter().map(|x| display_value("formula", config, x)).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", "),
            Value::Number(_) => display_value("number", config, value),
            Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            Value::String(s) => s.clone(),
            _ => String::new(),
        },
        "relation" => value.as_array().map(|a| a.len().to_string() + " linked").unwrap_or_default(),
        "select" | "status" => value.as_str().and_then(|id| option_name(config, id)).unwrap_or_default(),
        "multi_select" => value
            .as_array()
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| id.as_str().and_then(|id| option_name(config, id)))
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default(),
        "date" => {
            let start = value.get("start").and_then(|s| s.as_str()).unwrap_or("");
            match value.get("end").and_then(|e| e.as_str()) {
                Some(end) => format!("{start} → {end}"),
                None => start.to_string(),
            }
        }
        "checkbox" => if value.as_bool() == Some(true) { "true" } else { "false" }.to_string(),
        "files" => value
            .as_array()
            .map(|f| f.iter().filter_map(|x| x.get("name").and_then(|n| n.as_str())).collect::<Vec<_>>().join("; "))
            .unwrap_or_default(),
        "number" => match value.as_f64() {
            Some(n) if n.fract() == 0.0 && n.abs() < 1e15 => format!("{}", n as i64),
            Some(n) => n.to_string(),
            None => String::new(),
        },
        _ => match value {
            Value::String(s) => s.clone(),
            Value::Null => String::new(),
            other => other.to_string(),
        },
    }
}

pub struct Conversion {
    pub value: Option<Value>,
    /// The old value could not be represented and is dropped.
    pub lost: bool,
}

/// Converts a stored value between property types. New select options are appended to `new_config`.
pub fn convert_value(from: &str, from_config: &Value, to: &str, new_config: &mut Value, value: &Value) -> Conversion {
    let text = display_value(from, from_config, value);
    let keep = |v: Value| Conversion { value: Some(v), lost: false };
    let lost = Conversion { value: None, lost: true };
    match to {
        "text" | "url" => {
            if text.is_empty() { Conversion { value: None, lost: false } } else { keep(json!(text)) }
        }
        "email" => if text.contains('@') { keep(json!(text)) } else { lost },
        "number" => match from {
            "number" => keep(value.clone()),
            "checkbox" => keep(json!(if value.as_bool() == Some(true) { 1 } else { 0 })),
            _ => text.trim().parse::<f64>().map(|n| keep(json!(n))).unwrap_or(lost),
        },
        "checkbox" => match text.trim().to_lowercase().as_str() {
            "true" | "yes" | "1" | "✓" | "x" | "ใช่" => keep(json!(true)),
            "" | "false" | "no" | "0" => Conversion { value: None, lost: false },
            _ => lost,
        },
        "date" => {
            let t = text.get(0..10).unwrap_or("");
            if is_valid_date(t) { keep(json!({ "start": t })) } else { lost }
        }
        "select" | "status" | "multi_select" => {
            let names: Vec<String> = match from {
                "multi_select" => text.split("; ").map(String::from).collect(),
                _ if to == "multi_select" => text.split([',', ';']).map(|s| s.trim().to_string()).collect(),
                _ => vec![text.trim().to_string()],
            };
            let names: Vec<String> = names.into_iter().filter(|n| !n.is_empty()).collect();
            if names.is_empty() {
                return Conversion { value: None, lost: false };
            }
            let mut ids = vec![];
            for name in &names {
                ids.push(json!(ensure_option(new_config, name)));
            }
            if to == "multi_select" {
                keep(Value::Array(ids))
            } else {
                Conversion { value: Some(ids[0].clone()), lost: names.len() > 1 }
            }
        }
        _ => lost,
    }
}

const OPTION_COLORS: &[&str] = &["gray", "brown", "orange", "yellow", "green", "blue", "purple", "pink", "red"];

/// Returns the id of the option named `name`, creating it if needed.
pub fn ensure_option(config: &mut Value, name: &str) -> String {
    let mut opts = options(config);
    if let Some(o) = opts.iter().find(|o| o.get("name").and_then(|n| n.as_str()) == Some(name)) {
        return o["id"].as_str().unwrap_or_default().to_string();
    }
    let id = crate::domain::new_id();
    let color = OPTION_COLORS[opts.len() % OPTION_COLORS.len()];
    opts.push(json!({ "id": id, "name": name, "color": color }));
    if !config.is_object() {
        *config = json!({});
    }
    config["options"] = Value::Array(opts);
    id
}

pub fn default_status_config() -> Value {
    json!({ "options": [
        { "id": crate::domain::new_id(), "name": "To do", "color": "gray", "group": "todo" },
        { "id": crate::domain::new_id(), "name": "In progress", "color": "blue", "group": "in_progress" },
        { "id": crate::domain::new_id(), "name": "Done", "color": "green", "group": "done" }
    ]})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_values() {
        let cfg = json!({});
        assert!(validate_value("number", &cfg, &json!("12.5")).unwrap().is_some());
        assert!(validate_value("number", &cfg, &json!("abc")).is_err());
        assert!(validate_value("email", &cfg, &json!("not-email")).is_err());
        assert!(validate_value("date", &cfg, &json!({"start": "2026-10-21"})).is_ok());
        assert!(validate_value("date", &cfg, &json!({"start": "21/10/2026"})).is_err());
        assert!(validate_value("select", &cfg, &json!("missing")).is_err());
    }

    #[test]
    fn converts_text_to_select_and_number() {
        let mut cfg = json!({});
        let c = convert_value("text", &json!({}), "select", &mut cfg, &json!("ด่วน"));
        assert!(!c.lost);
        assert_eq!(option_name(&cfg, c.value.unwrap().as_str().unwrap()).unwrap(), "ด่วน");
        let n = convert_value("text", &json!({}), "number", &mut json!({}), &json!("ไม่ใช่ตัวเลข"));
        assert!(n.lost);
    }
}
