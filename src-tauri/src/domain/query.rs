//! Database view filtering (nested AND/OR groups) and multi-level sorting.

use serde_json::{Map, Value};
use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct PropDef {
    pub id: String,
    pub name: String,
    pub ptype: String,
    pub config: Value,
}

#[derive(Clone, Debug)]
pub struct RecordData {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub position: f64,
    pub created_at: String,
    pub updated_at: String,
    /// created/updated converted to local dates in the user's time zone.
    pub created_date: String,
    pub updated_date: String,
    pub values: Map<String, Value>,
}

impl RecordData {
    pub fn value_of(&self, p: &PropDef) -> Value {
        match p.ptype.as_str() {
            "title" => Value::String(self.title.clone()),
            "created_time" => Value::String(self.created_at.clone()),
            "last_edited_time" => Value::String(self.updated_at.clone()),
            _ => self.values.get(&p.id).cloned().unwrap_or(Value::Null),
        }
    }

    fn date_of(&self, p: &PropDef) -> Option<String> {
        match p.ptype.as_str() {
            "created_time" => Some(self.created_date.clone()),
            "last_edited_time" => Some(self.updated_date.clone()),
            _ => self.values.get(&p.id)?.get("start")?.as_str().map(|s| s.chars().take(10).collect()),
        }
    }
}

fn is_empty(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Bool(b) => !b,
        _ => false,
    }
}

/// Evaluates a filter node: either a group `{op, conditions}` or a condition `{propertyId, operator, value}`.
pub fn matches(filter: &Value, rec: &RecordData, props: &HashMap<String, PropDef>) -> bool {
    if let Some(conds) = filter.get("conditions").and_then(|c| c.as_array()) {
        if conds.is_empty() {
            return true;
        }
        let any = filter.get("op").and_then(|o| o.as_str()) == Some("or");
        return if any {
            conds.iter().any(|c| matches(c, rec, props))
        } else {
            conds.iter().all(|c| matches(c, rec, props))
        };
    }
    let Some(p) = filter.get("propertyId").and_then(|p| p.as_str()).and_then(|id| props.get(id)) else {
        return true; // a condition on a deleted property never hides data
    };
    let op = filter.get("operator").and_then(|o| o.as_str()).unwrap_or("");
    let arg = filter.get("value").cloned().unwrap_or(Value::Null);
    let v = rec.value_of(p);
    match op {
        "is_empty" => return is_empty(&v),
        "is_not_empty" => return !is_empty(&v),
        _ => {}
    }
    // A condition still being edited (no value yet) does not hide anything.
    if arg.is_null() || arg.as_str() == Some("") {
        return true;
    }
    match p.ptype.as_str() {
        "title" | "text" | "url" | "email" => {
            let s = v.as_str().unwrap_or("").to_lowercase();
            let a = arg.as_str().unwrap_or("").to_lowercase();
            match op {
                "contains" => s.contains(&a),
                "not_contains" => !s.contains(&a),
                "equals" => s == a,
                "not_equals" => s != a,
                "starts_with" => s.starts_with(&a),
                "ends_with" => s.ends_with(&a),
                _ => true,
            }
        }
        "number" => {
            let (Some(n), Some(a)) = (v.as_f64(), arg.as_f64().or_else(|| arg.as_str()?.parse().ok())) else {
                return op == "neq";
            };
            match op {
                "eq" => n == a,
                "neq" => n != a,
                "gt" => n > a,
                "gte" => n >= a,
                "lt" => n < a,
                "lte" => n <= a,
                _ => true,
            }
        }
        "select" | "status" => {
            let s = v.as_str().unwrap_or("");
            match op {
                "is" => Some(s) == arg.as_str(),
                "is_not" => Some(s) != arg.as_str(),
                "is_any_of" => arg.as_array().is_some_and(|a| a.iter().any(|x| x.as_str() == Some(s))),
                _ => true,
            }
        }
        "multi_select" => {
            let has = v.as_array().is_some_and(|a| a.iter().any(|x| x.as_str() == arg.as_str()));
            match op {
                "contains" => has,
                "not_contains" => !has,
                _ => true,
            }
        }
        "checkbox" => {
            let checked = v.as_bool() == Some(true);
            op != "is" || checked == arg.as_bool().unwrap_or(true)
        }
        "date" | "created_time" | "last_edited_time" => {
            let Some(d) = rec.date_of(p) else { return false };
            let a = arg.as_str().unwrap_or("");
            match op {
                "is" => d == a,
                "before" => d.as_str() < a,
                "after" => d.as_str() > a,
                "on_or_before" => d.as_str() <= a,
                "on_or_after" => d.as_str() >= a,
                _ => true,
            }
        }
        _ => true,
    }
}

fn option_index(p: &PropDef, id: &str) -> usize {
    crate::domain::properties::options(&p.config)
        .iter()
        .position(|o| o.get("id").and_then(|v| v.as_str()) == Some(id))
        .unwrap_or(usize::MAX)
}

fn compare_values(p: &PropDef, a: &RecordData, b: &RecordData) -> Ordering {
    let (va, vb) = (a.value_of(p), b.value_of(p));
    // Empty values always sort last regardless of direction is handled by the caller.
    match p.ptype.as_str() {
        "number" => va.as_f64().partial_cmp(&vb.as_f64()).unwrap_or(Ordering::Equal),
        "checkbox" => va.as_bool().unwrap_or(false).cmp(&vb.as_bool().unwrap_or(false)),
        "select" | "status" => option_index(p, va.as_str().unwrap_or("")).cmp(&option_index(p, vb.as_str().unwrap_or(""))),
        "multi_select" => {
            let first = |v: &Value| v.as_array().and_then(|x| x.first()).and_then(|x| x.as_str()).map(|id| option_index(p, id));
            first(&va).cmp(&first(&vb))
        }
        "date" | "created_time" | "last_edited_time" => a.date_of(p).cmp(&b.date_of(p)).then_with(|| {
            va.as_str().unwrap_or("").cmp(vb.as_str().unwrap_or(""))
        }),
        _ => {
            let sa = crate::domain::properties::display_value(&p.ptype, &p.config, &va).to_lowercase();
            let sb = crate::domain::properties::display_value(&p.ptype, &p.config, &vb).to_lowercase();
            sa.cmp(&sb)
        }
    }
}

pub fn sort(records: &mut [RecordData], sorts: &[Value], props: &HashMap<String, PropDef>) {
    let specs: Vec<(&PropDef, bool)> = sorts
        .iter()
        .filter_map(|s| {
            let p = props.get(s.get("propertyId")?.as_str()?)?;
            Some((p, s.get("direction").and_then(|d| d.as_str()) == Some("desc")))
        })
        .collect();
    records.sort_by(|a, b| {
        for (p, desc) in &specs {
            let (ea, eb) = (is_empty(&a.value_of(p)), is_empty(&b.value_of(p)));
            let ord = match (ea, eb) {
                (true, true) => Ordering::Equal,
                (true, false) => return Ordering::Greater,
                (false, true) => return Ordering::Less,
                _ => {
                    let o = compare_values(p, a, b);
                    if *desc { o.reverse() } else { o }
                }
            };
            if ord != Ordering::Equal {
                return ord;
            }
        }
        a.position.partial_cmp(&b.position).unwrap_or(Ordering::Equal)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rec(id: &str, title: &str, n: f64, pos: f64) -> RecordData {
        let mut values = Map::new();
        values.insert("num".into(), json!(n));
        RecordData {
            id: id.into(),
            title: title.into(),
            icon: None,
            position: pos,
            created_at: "2026-10-21T00:00:00.000Z".into(),
            updated_at: "2026-10-21T00:00:00.000Z".into(),
            created_date: "2026-10-21".into(),
            updated_date: "2026-10-21".into(),
            values,
        }
    }

    fn props() -> HashMap<String, PropDef> {
        let mut m = HashMap::new();
        m.insert("t".into(), PropDef { id: "t".into(), name: "ชื่อ".into(), ptype: "title".into(), config: json!({}) });
        m.insert("num".into(), PropDef { id: "num".into(), name: "n".into(), ptype: "number".into(), config: json!({}) });
        m
    }

    #[test]
    fn and_or_filters() {
        let p = props();
        let r = rec("1", "งานออกแบบ", 5.0, 0.0);
        let f = json!({"op": "or", "conditions": [
            {"propertyId": "num", "operator": "gt", "value": 10},
            {"op": "and", "conditions": [
                {"propertyId": "t", "operator": "contains", "value": "ออกแบบ"},
                {"propertyId": "num", "operator": "lte", "value": 5}
            ]}
        ]});
        assert!(matches(&f, &r, &p));
        let f2 = json!({"op": "and", "conditions": [{"propertyId": "num", "operator": "gt", "value": 10}]});
        assert!(!matches(&f2, &r, &p));
    }

    #[test]
    fn multi_level_sort() {
        let p = props();
        let mut rs = vec![rec("a", "B", 1.0, 0.0), rec("b", "A", 1.0, 1.0), rec("c", "C", 0.0, 2.0)];
        sort(&mut rs, &[json!({"propertyId": "num", "direction": "desc"}), json!({"propertyId": "t"})], &p);
        let ids: Vec<_> = rs.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "a", "c"]);
    }
}
