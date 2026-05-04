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

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            return OpReport {
                summary: format!("Failed to read CSV: {}", e),
                affected: 0,
                is_error: true,
            }
        }
    };
    let first_line = content.lines().next().unwrap_or("");
    let delim = if first_line.matches(';').count() > first_line.matches(',').count() {
        b';'
    } else {
        b','
    };

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .from_reader(content.as_bytes());

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

pub fn roofer2multiroofs(doc: &mut CityJsonDocument) -> OpReport {
    // If CityJSONSeq, collapse to unified CityJSON first so we operate on one CityObjects map
    if !doc.features.is_empty() {
        let collapsed = io::collapse(doc);
        doc.header = collapsed.as_object().cloned().unwrap_or_default();
        doc.features.clear();
    }

    let city_objects = doc
        .header
        .get_mut("CityObjects")
        .and_then(|v| v.as_object_mut());

    let city_objects = match city_objects {
        Some(c) => c,
        None => {
            return OpReport {
                summary: "No CityObjects in file".to_string(),
                affected: 0,
                is_error: true,
            }
        }
    };

    // Step 1: collect BuildingPart IDs and their parent mappings
    let part_ids: Vec<String> = city_objects
        .iter()
        .filter(|(_, v)| {
            v.get("type").and_then(|t| t.as_str()) == Some("BuildingPart")
        })
        .map(|(k, _)| k.clone())
        .collect();

    if part_ids.is_empty() {
        return OpReport {
            summary: "No BuildingPart objects found".to_string(),
            affected: 0,
            is_error: true,
        };
    }

    let mut parent_to_children: HashMap<String, Vec<String>> = HashMap::new();
    for id in &part_ids {
        if let Some(obj) = city_objects.get(id) {
            if let Some(parents) = obj.get("parents").and_then(|v| v.as_array()) {
                for p in parents {
                    if let Some(pid) = p.as_str() {
                        parent_to_children
                            .entry(pid.to_string())
                            .or_default()
                            .push(id.clone());
                    }
                }
            }
        }
    }

    // Step 2: collect geometries to transfer from BuildingParts to parents
    let mut parent_geometries: HashMap<String, Vec<Value>> = HashMap::new();
    for (parent_id, child_ids) in &parent_to_children {
        let mut geoms = Vec::new();
        for cid in child_ids {
            if let Some(child) = city_objects.get(cid) {
                if let Some(arr) = child.get("geometry").and_then(|v| v.as_array()) {
                    geoms.extend(arr.iter().cloned());
                }
            }
        }
        parent_geometries.insert(parent_id.clone(), geoms);
    }

    // Step 3: modify parents — remove lod=0, add child geometries, remove children field
    for (parent_id, new_geoms) in &parent_geometries {
        if let Some(parent) = city_objects.get_mut(parent_id) {
            if let Some(geoms) = parent.get_mut("geometry").and_then(|v| v.as_array_mut()) {
                geoms.retain(|g| g.get("lod").and_then(|v| v.as_str()) != Some("0"));
                geoms.extend(new_geoms.iter().cloned());
            } else {
                if !new_geoms.is_empty() {
                    parent
                        .as_object_mut()
                        .map(|m| m.insert("geometry".to_string(), Value::Array(new_geoms.clone())));
                }
            }
            parent.as_object_mut().map(|m| m.remove("children"));
        }
    }

    // Step 4: remove all BuildingParts
    for id in &part_ids {
        city_objects.remove(id);
    }

    // Step 5: rename b3_volume → +building-volume
    let mut rename_count = 0;
    for (_id, obj) in city_objects.iter_mut() {
        if let Some(attrs) = obj.get_mut("attributes").and_then(|v| v.as_object_mut()) {
            if let Some(val) = attrs.remove("b3_volume") {
                attrs.insert("+building-volume".to_string(), val);
                rename_count += 1;
            }
        }
    }

    // Step 6: add multiroofs extension (append, don't overwrite)
    let ext_name = "multiroofs";
    let ext_value = serde_json::json!({
        "url": "https://raw.githubusercontent.com/MultiRoofs/cityjson-extension/refs/heads/main/multiroofs.ext.json",
        "version": "0.1.0"
    });

    if let Some(exts) = doc.header.get_mut("extensions").and_then(|v| v.as_object_mut()) {
        if !exts.contains_key(ext_name) {
            exts.insert(ext_name.to_string(), ext_value);
        }
    } else {
        let mut exts = Map::new();
        exts.insert(ext_name.to_string(), ext_value);
        doc.header
            .insert("extensions".to_string(), Value::Object(exts));
    }

    let summary = format!(
        "Roofer→MultiRoofs: merged {} BuildingPart(s), removed lod=0 geometry, renamed {} attribute(s), added extension",
        part_ids.len(),
        rename_count,
    );

    OpReport {
        summary,
        affected: part_ids.len(),
        is_error: part_ids.is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io;

    #[test]
    fn test_roofer2multiroofs() {
        let mut doc = io::read_file("data/roofer_output_b2.city.json").unwrap();
        let report = roofer2multiroofs(&mut doc);
        assert!(!report.is_error, "Operation failed: {}", report.summary);
        assert!(report.affected > 0, "No BuildingParts were processed");

        // Verify: no BuildingParts remain
        for (id, obj) in io::get_all_city_objects(&doc) {
            let ty = obj.get("type").and_then(|v| v.as_str()).unwrap();
            assert!(
                ty != "BuildingPart",
                "BuildingPart '{}' should have been removed",
                id
            );
        }

        // Verify: remaining objects have no children field, no lod=0 geometry
        let expected_ids = ["6", "20"];
        let mut found = std::collections::HashSet::new();
        for (id, obj) in io::get_all_city_objects(&doc) {
            found.insert(id.clone());
            assert!(
                obj.get("children").is_none(),
                "Object '{}' should have no children field",
                id
            );
            if let Some(geoms) = obj.get("geometry").and_then(|v| v.as_array()) {
                for g in geoms {
                    let lod = g.get("lod").and_then(|v| v.as_str()).unwrap();
                    assert_ne!(lod, "0", "Object '{}' has lod=0 geometry", id);
                }
                assert_eq!(geoms.len(), 1, "Object '{}' should have exactly 1 geometry", id);
            }
        }
        for eid in &expected_ids {
            assert!(found.contains(*eid), "Expected object '{}' not found", eid);
        }

        // Verify: extension added
        let exts = doc.header.get("extensions").and_then(|v| v.as_object());
        assert!(exts.is_some(), "extensions should exist");
        let multiroofs = exts
            .unwrap()
            .get("multiroofs")
            .and_then(|v| v.as_object());
        assert!(multiroofs.is_some(), "multiroofs extension should exist");

        // Verify: output matches expected file
        let mut expected_doc = io::read_file("data/roofer_corrected_b2.city.json").unwrap();
        // Align extensions in expected doc for comparison
        let ext_val = doc.header.get("extensions").cloned();
        expected_doc
            .header
            .insert("extensions".to_string(), ext_val.unwrap());
        let result_json =
            serde_json::to_string_pretty(&io::collapse(&doc)).unwrap();
        let expected_json =
            serde_json::to_string_pretty(&io::collapse(&expected_doc)).unwrap();
        assert_eq!(result_json, expected_json, "Output does not match expected");
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
