use std::collections::{BTreeMap, HashMap};

use crate::io;
use crate::model::{CityJsonDocument, CITY_OBJECT_TYPES};

#[derive(Debug, Clone)]
pub struct FileStats {
    pub format_name: String,
    pub version: String,
    pub total_objects: usize,
    pub objects_with_attrs: usize,
    pub total_vertices: usize,
    pub object_type_counts: BTreeMap<String, usize>,
    pub other_object_types: BTreeMap<String, usize>,
    pub attribute_inventory: Vec<(String, usize, String)>,
    pub extensions: Vec<(String, String)>,
    pub crs: String,
}

fn fmt_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => {
            if s.len() > 40 {
                format!("\"{}…\"", &s[..37])
            } else {
                format!("\"{}\"", s)
            }
        }
        serde_json::Value::Object(_) => "{…}".to_string(),
        serde_json::Value::Array(_) => "[…]".to_string(),
    }
}

pub fn compute_stats(doc: &CityJsonDocument) -> FileStats {
    let format_name = if doc.original_format == crate::model::InputFormat::CityJSONSeq {
        "CityJSONSeq"
    } else {
        "CityJSON"
    };

    let version = doc
        .header
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let total_vertices = doc
        .header
        .get("vertices")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
        + doc
            .features
            .iter()
            .map(|f| {
                f.get("vertices")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0)
            })
            .sum::<usize>();

    let mut type_counts: HashMap<String, usize> = HashMap::new();
    let mut attr_counts: HashMap<String, usize> = HashMap::new();
    let mut attr_samples: HashMap<String, String> = HashMap::new();
    let mut total_objects = 0;
    let mut objects_with_attrs = 0;

    for (_id, obj) in io::get_all_city_objects(doc) {
        total_objects += 1;

        if let Some(ty) = obj.get("type").and_then(|v| v.as_str()) {
            *type_counts.entry(ty.to_string()).or_insert(0) += 1;
        }

        if let Some(attrs) = obj.get("attributes").and_then(|v| v.as_object()) {
            objects_with_attrs += 1;
            for (key, val) in attrs {
                attr_counts
                    .entry(key.clone())
                    .and_modify(|c| *c += 1)
                    .or_insert(1);
                attr_samples
                    .entry(key.clone())
                    .or_insert_with(|| fmt_value(val));
            }
        }
    }

    let known_types: Vec<String> = CITY_OBJECT_TYPES.iter().map(|s| s.to_string()).collect();
    let mut object_type_counts = BTreeMap::new();
    let mut other_object_types = BTreeMap::new();

    for (ty, count) in &type_counts {
        if known_types.contains(ty) {
            object_type_counts.insert(ty.clone(), *count);
        } else {
            other_object_types.insert(ty.clone(), *count);
        }
    }

    let mut attribute_inventory: Vec<(String, usize, String)> = attr_counts
        .into_iter()
        .map(|(k, c)| {
            let sample = attr_samples.get(&k).cloned().unwrap_or_default();
            (k, c, sample)
        })
        .collect();
    attribute_inventory.sort_by(|a, b| a.0.cmp(&b.0));

    let extensions: Vec<(String, String)> = doc
        .header
        .get("extensions")
        .and_then(|v| v.as_object())
        .map(|o| {
            o.iter()
                .map(|(k, v)| {
                    let url = v
                        .get("url")
                        .and_then(|u| u.as_str())
                        .unwrap_or("")
                        .to_string();
                    (k.clone(), url)
                })
                .collect()
        })
        .unwrap_or_default();

    let crs = doc
        .header
        .get("metadata")
        .and_then(|m| m.as_object())
        .and_then(|m| m.get("referenceSystem"))
        .and_then(|v| v.as_str())
        .unwrap_or("none")
        .to_string();

    FileStats {
        format_name: format_name.to_string(),
        version,
        total_objects,
        objects_with_attrs,
        total_vertices,
        object_type_counts,
        other_object_types,
        attribute_inventory,
        extensions,
        crs,
    }
}
