use std::collections::HashMap;
use std::path::Path;
use serde_json::{Map, Value};

use crate::io;
use crate::model::CityJsonDocument;

#[derive(Debug, Clone)]
pub struct OpReport {
    pub summary: String,
    #[allow(dead_code)]
    pub affected: usize,
    pub is_error: bool,
}

pub fn remove_attribute(doc: &mut CityJsonDocument, attr_name: &str) -> OpReport {
    let mut count = 0;
    for (_id, obj) in io::get_all_city_objects_mut(doc) {
        if let Some(attrs) = obj
            .get_mut("attributes")
            .and_then(|v| v.as_object_mut())
        {
            if attrs.remove(attr_name).is_some() {
                count += 1;
            }
        }
    }
    OpReport {
        summary: format!("Removed attribute '{}' from {} object(s)", attr_name, count),
        affected: count,
        is_error: count == 0,
    }
}

pub fn rename_attribute(doc: &mut CityJsonDocument, old_name: &str, new_name: &str) -> OpReport {
    if old_name == new_name {
        return OpReport {
            summary: "Old and new names are identical".to_string(),
            affected: 0,
            is_error: true,
        };
    }
    let mut count = 0;
    for (_id, obj) in io::get_all_city_objects_mut(doc) {
        if let Some(attrs) = obj
            .get_mut("attributes")
            .and_then(|v| v.as_object_mut())
        {
            if let Some(val) = attrs.remove(old_name) {
                attrs.insert(new_name.to_string(), val);
                count += 1;
            }
        }
    }
    OpReport {
        summary: format!(
            "Renamed attribute '{}' → '{}' in {} object(s)",
            old_name, new_name, count
        ),
        affected: count,
        is_error: count == 0,
    }
}

pub fn add_attributes_from_csv(doc: &mut CityJsonDocument, csv_path: &str) -> OpReport {
    let path = Path::new(csv_path);
    if !path.exists() {
        return OpReport {
            summary: format!("File not found: {}", csv_path),
            affected: 0,
            is_error: true,
        };
    }

    let mut rdr = match csv::Reader::from_path(path) {
        Ok(r) => r,
        Err(e) => {
            return OpReport {
                summary: format!("Failed to read CSV: {}", e),
                affected: 0,
                is_error: true,
            }
        }
    };

    let headers = match rdr.headers() {
        Ok(h) => h.clone(),
        Err(e) => {
            return OpReport {
                summary: format!("Failed to read CSV headers: {}", e),
                affected: 0,
                is_error: true,
            }
        }
    };

    let attr_names: Vec<String> = headers.iter().skip(1).map(|s| s.to_string()).collect();
    // Build ID→CityObject lookup
    let mut id_map: HashMap<String, usize> = HashMap::new();
    let objects = io::get_all_city_objects_mut(doc);
    for (i, (id, _obj)) in objects.iter().enumerate() {
        id_map.insert(id.clone(), i);
    }
    drop(objects); // release borrow

    let mut updated_count = 0;
    let mut error_count = 0;
    let mut errors = Vec::new();

    for (row_idx, result) in rdr.records().enumerate() {
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                errors.push(format!("Row {}: {}", row_idx + 2, e));
                error_count += 1;
                continue;
            }
        };

        let obj_id = record.get(0).unwrap_or_default().to_string();
        let obj_index = match id_map.get(&obj_id) {
            Some(&i) => i,
            None => {
                errors.push(format!(
                    "Row {}: CityObject '{}' not found",
                    row_idx + 2,
                    obj_id
                ));
                error_count += 1;
                continue;
            }
        };

        // Re-borrow to modify the object
        let objects2 = io::get_all_city_objects_mut(doc);
        if let Some((_id, obj)) = objects2.into_iter().nth(obj_index) {
            let attrs = obj
                .get_mut("attributes")
                .and_then(|v| v.as_object_mut());
            if let Some(attrs) = attrs {
                for (i, attr_name) in attr_names.iter().enumerate() {
                    let val_str = record.get(i + 1).unwrap_or_default();
                    let val = parse_csv_value(val_str);
                    attrs.insert(attr_name.clone(), val);
                }
                updated_count += 1;
            } else {
                // Object doesn't have an attributes field — create one
                let mut new_attrs = Map::new();
                for (i, attr_name) in attr_names.iter().enumerate() {
                    let val_str = record.get(i + 1).unwrap_or_default();
                    let val = parse_csv_value(val_str);
                    new_attrs.insert(attr_name.clone(), val);
                }
                obj.as_object_mut()
                    .map(|m| m.insert("attributes".to_string(), Value::Object(new_attrs)));
                updated_count += 1;
            }
        }
    }

    let mut summary = format!(
        "Added attributes to {} object(s) from '{}'",
        updated_count,
        csv_path
    );
    if error_count > 0 {
        summary.push_str(&format!("\n{} error(s):", error_count));
        for e in errors {
            summary.push_str(&format!("\n  {}", e));
        }
    }

    OpReport {
        summary,
        affected: updated_count,
        is_error: updated_count == 0,
    }
}

fn parse_csv_value(s: &str) -> Value {
    let s = s.trim();
    if s.is_empty() {
        return Value::Null;
    }
    // Try integer
    if let Ok(n) = s.parse::<i64>() {
        return Value::Number(serde_json::Number::from(n));
    }
    // Try float
    if let Ok(n) = s.parse::<f64>() {
        if let Some(num) = serde_json::Number::from_f64(n) {
            return Value::Number(num);
        }
    }
    // Try boolean
    if s.eq_ignore_ascii_case("true") {
        return Value::Bool(true);
    }
    if s.eq_ignore_ascii_case("false") {
        return Value::Bool(false);
    }
    // String
    Value::String(s.to_string())
}
