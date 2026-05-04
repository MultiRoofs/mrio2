use std::collections::{HashMap, HashSet};
use std::fs;
use serde_json::{Map, Number, Value};

use crate::model::*;

pub fn read_file(path: &str) -> Result<CityJsonDocument, String> {
    let input_format = InputFormat::from_path(path);
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read '{}': {}", path, e))?;

    match input_format {
        InputFormat::CityJSON => read_cityjson(&content),
        InputFormat::CityJSONSeq => read_cityjsonseq(&content),
    }
}

fn read_cityjson(content: &str) -> Result<CityJsonDocument, String> {
    let value: Value =
        serde_json::from_str(content).map_err(|e| format!("Invalid JSON: {}", e))?;
    let header = value
        .as_object()
        .ok_or_else(|| "Root is not a JSON object".to_string())?
        .clone();
    Ok(CityJsonDocument {
        header,
        features: vec![],
        original_format: InputFormat::CityJSON,
    })
}

fn read_cityjsonseq(content: &str) -> Result<CityJsonDocument, String> {
    let mut lines = content.lines().filter(|l| !l.trim().is_empty());
    let first = lines
        .next()
        .ok_or_else(|| "Empty file".to_string())?;
    let header: Map<String, Value> = serde_json::from_str(first)
        .map_err(|e| format!("Invalid JSON in header: {}", e))?;

    let mut features = Vec::new();
    for line in lines {
        let feature: Map<String, Value> = serde_json::from_str(line)
            .map_err(|e| format!("Invalid JSON in feature: {}", e))?;
        features.push(feature);
    }

    Ok(CityJsonDocument {
        header,
        features,
        original_format: InputFormat::CityJSONSeq,
    })
}

pub fn write_file(path: &str, doc: &CityJsonDocument, format: OutputFormat) -> Result<(), String> {
    let content = match format {
        OutputFormat::CityJSON => serialize_cityjson(doc),
        OutputFormat::CityJSONSeq => serialize_cityjsonseq(doc),
    }?;
    fs::write(path, content).map_err(|e| format!("Failed to write '{}': {}", path, e))?;
    Ok(())
}

fn serialize_cityjson(doc: &CityJsonDocument) -> Result<String, String> {
    let merged = collapse(doc);
    serde_json::to_string_pretty(&merged)
        .map_err(|e| format!("Serialization error: {}", e))
}

fn serialize_cityjsonseq(doc: &CityJsonDocument) -> Result<String, String> {
    let (header, features) = expand(doc);
    let header_line =
        serde_json::to_string(&header).map_err(|e| format!("Serialization error: {}", e))?;
    let mut lines = vec![header_line];
    for f in features {
        let line =
            serde_json::to_string(&f).map_err(|e| format!("Serialization error: {}", e))?;
        lines.push(line);
    }
    Ok(lines.join("\n"))
}

/// Merge all features into a single CityJSON object.
pub fn collapse(doc: &CityJsonDocument) -> Value {
    let mut result = doc.header.clone();
    if doc.features.is_empty() {
        return Value::Object(result);
    }

    let mut all_objects: Map<String, Value> = result
        .get("CityObjects")
        .and_then(|v| v.as_object())
        .map(|o| o.clone())
        .unwrap_or_default();

    let mut all_vertices: Vec<Value> = result
        .get("vertices")
        .and_then(|v| v.as_array())
        .map(|a| a.clone())
        .unwrap_or_default();

    let mut vertex_offset = all_vertices.len();

    for feature in &doc.features {
        if let Some(objects) = feature.get("CityObjects").and_then(|v| v.as_object()) {
            for (id, obj) in objects {
                let mut obj = obj.clone();
                if vertex_offset > 0 {
                    remap_geometry_vertices(&mut obj, vertex_offset);
                }
                all_objects.insert(id.clone(), obj);
            }
        }

        if let Some(verts) = feature.get("vertices").and_then(|v| v.as_array()) {
            for v in verts {
                all_vertices.push(v.clone());
            }
        }

        vertex_offset = all_vertices.len();
    }

    result.insert("CityObjects".to_string(), Value::Object(all_objects));
    result.insert("vertices".to_string(), Value::Array(all_vertices));
    result.insert("type".to_string(), Value::String("CityJSON".to_string()));
    Value::Object(result)
}

/// Split a single CityJSON object into a header + features for CityJSONSeq.
pub fn expand(doc: &CityJsonDocument) -> (Value, Vec<Value>) {
    let empty_objects = Value::Object(Map::new());
    let empty_vertices = Value::Array(vec![]);

    let all_objects: Vec<(String, Value)> = match doc.header.get("CityObjects") {
        Some(Value::Object(map)) => map.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        _ => vec![],
    };

    let all_vertices: Vec<Value> = match doc.header.get("vertices") {
        Some(Value::Array(arr)) => arr.clone(),
        _ => vec![],
    };

    let mut header = doc.header.clone();
    header.insert("CityObjects".to_string(), empty_objects.clone());
    header.insert("vertices".to_string(), empty_vertices.clone());

    let mut features = Vec::new();
    if all_objects.is_empty() && !doc.features.is_empty() {
        for f in &doc.features {
            features.push(Value::Object(f.clone()));
        }
    } else if !all_objects.is_empty() {
        for (id, obj) in &all_objects {
            let used = collect_vertex_indices(obj);
            let mut local_vertices = Vec::new();
            let mut global_to_local: HashMap<usize, usize> = HashMap::new();
            let mut sorted: Vec<usize> = used.into_iter().collect();
            sorted.sort();
            for (local_idx, global_idx) in sorted.iter().enumerate() {
                if *global_idx < all_vertices.len() {
                    local_vertices.push(all_vertices[*global_idx].clone());
                    global_to_local.insert(*global_idx, local_idx);
                }
            }

            let mut obj = obj.clone();
            remap_geometry_vertices_with_map(&mut obj, &global_to_local);

            let mut co = Map::new();
            co.insert(id.clone(), obj);

            let mut feature = Map::new();
            feature.insert(
                "type".to_string(),
                Value::String("CityJSONFeature".to_string()),
            );
            feature.insert("id".to_string(), Value::String(id.clone()));
            feature.insert("CityObjects".to_string(), Value::Object(co));
            feature.insert("vertices".to_string(), Value::Array(local_vertices));

            features.push(Value::Object(feature));
        }
    }

    (Value::Object(header), features)
}

fn remap_geometry_vertices(obj: &mut Value, offset: usize) {
    let geometries = obj
        .get_mut("geometry")
        .and_then(|v| v.as_array_mut());
    if let Some(geoms) = geometries {
        for geom in geoms {
            let geom_type = geom
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if let Some(b) = geom.get_mut("boundaries") {
                add_offset_to_boundaries(b, &geom_type, offset);
            }
        }
    }
}

fn remap_geometry_vertices_with_map(obj: &mut Value, remap: &HashMap<usize, usize>) {
    let geometries = obj
        .get_mut("geometry")
        .and_then(|v| v.as_array_mut());
    if let Some(geoms) = geometries {
        for geom in geoms {
            let geom_type = geom
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if let Some(b) = geom.get_mut("boundaries") {
                remap_boundaries_with_map(b, &geom_type, remap);
            }
        }
    }
}

fn add_offset_to_boundaries(val: &mut Value, geom_type: &str, offset: usize) {
    let val_ref = val;
    match geom_type {
        "MultiPoint" => {
            if let Some(arr) = val_ref.as_array_mut() {
                for item in arr.iter_mut() {
                    if let Some(n) = item.as_i64() {
                        *item = Value::Number(Number::from(n as usize + offset));
                    }
                }
            }
        }
        "MultiLineString" => {
            if let Some(arr) = val_ref.as_array_mut() {
                for line in arr.iter_mut() {
                    add_to_vertex_array(line, offset);
                }
            }
        }
        "MultiSurface" | "CompositeSurface" => {
            if let Some(arr) = val_ref.as_array_mut() {
                for surface in arr.iter_mut() {
                    add_to_surface(surface, offset);
                }
            }
        }
        "Solid" => {
            if let Some(arr) = val_ref.as_array_mut() {
                for shell in arr.iter_mut() {
                    if let Some(shell_arr) = shell.as_array_mut() {
                        for surface in shell_arr.iter_mut() {
                            add_to_surface(surface, offset);
                        }
                    }
                }
            }
        }
        "MultiSolid" | "CompositeSolid" => {
            if let Some(arr) = val_ref.as_array_mut() {
                for solid in arr.iter_mut() {
                    if let Some(solid_arr) = solid.as_array_mut() {
                        for shell in solid_arr.iter_mut() {
                            if let Some(shell_arr) = shell.as_array_mut() {
                                for surface in shell_arr.iter_mut() {
                                    add_to_surface(surface, offset);
                                }
                            }
                        }
                    }
                }
            }
        }
        "GeometryInstance" => {
            // GeometryInstance has boundaries = [vertex_index]
            if let Some(arr) = val_ref.as_array_mut() {
                for item in arr.iter_mut() {
                    if let Some(n) = item.as_i64() {
                        *item = Value::Number(Number::from(n as usize + offset));
                    }
                }
            }
        }
        _ => {}
    }
}

fn add_to_surface(val: &mut Value, offset: usize) {
    if let Some(rings) = val.as_array_mut() {
        for ring in rings.iter_mut() {
            add_to_vertex_array(ring, offset);
        }
    }
}

fn add_to_vertex_array(val: &mut Value, offset: usize) {
    if let Some(arr) = val.as_array_mut() {
        for item in arr.iter_mut() {
            if let Some(n) = item.as_i64() {
                *item = Value::Number(Number::from(n as usize + offset));
            }
        }
    }
}

fn remap_boundaries_with_map(val: &mut Value, geom_type: &str, remap: &HashMap<usize, usize>) {
    match geom_type {
        "MultiPoint" => {
            if let Some(arr) = val.as_array_mut() {
                for item in arr.iter_mut() {
                    if let Some(n) = item.as_i64() {
                        if let Some(&new) = remap.get(&(n as usize)) {
                            *item = Value::Number(Number::from(new));
                        }
                    }
                }
            }
        }
        "MultiLineString" => {
            if let Some(arr) = val.as_array_mut() {
                for line in arr.iter_mut() {
                    remap_vertex_array(line, remap);
                }
            }
        }
        "MultiSurface" | "CompositeSurface" => {
            if let Some(arr) = val.as_array_mut() {
                for surface in arr.iter_mut() {
                    remap_surface(surface, remap);
                }
            }
        }
        "Solid" => {
            if let Some(arr) = val.as_array_mut() {
                for shell in arr.iter_mut() {
                    if let Some(shell_arr) = shell.as_array_mut() {
                        for surface in shell_arr.iter_mut() {
                            remap_surface(surface, remap);
                        }
                    }
                }
            }
        }
        "MultiSolid" | "CompositeSolid" => {
            if let Some(arr) = val.as_array_mut() {
                for solid in arr.iter_mut() {
                    if let Some(solid_arr) = solid.as_array_mut() {
                        for shell in solid_arr.iter_mut() {
                            if let Some(shell_arr) = shell.as_array_mut() {
                                for surface in shell_arr.iter_mut() {
                                    remap_surface(surface, remap);
                                }
                            }
                        }
                    }
                }
            }
        }
        "GeometryInstance" => {
            if let Some(arr) = val.as_array_mut() {
                for item in arr.iter_mut() {
                    if let Some(n) = item.as_i64() {
                        if let Some(&new) = remap.get(&(n as usize)) {
                            *item = Value::Number(Number::from(new));
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn remap_surface(val: &mut Value, remap: &HashMap<usize, usize>) {
    if let Some(rings) = val.as_array_mut() {
        for ring in rings.iter_mut() {
            remap_vertex_array(ring, remap);
        }
    }
}

fn remap_vertex_array(val: &mut Value, remap: &HashMap<usize, usize>) {
    if let Some(arr) = val.as_array_mut() {
        for item in arr.iter_mut() {
            if let Some(n) = item.as_i64() {
                if let Some(&new) = remap.get(&(n as usize)) {
                    *item = Value::Number(Number::from(new));
                }
            }
        }
    }
}

fn collect_vertex_indices(obj: &Value) -> HashSet<usize> {
    let mut indices = HashSet::new();
    let geometries = obj
        .get("geometry")
        .and_then(|v| v.as_array());
    if let Some(geoms) = geometries {
        for geom in geoms {
            let geom_type = geom
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let boundaries = geom.get("boundaries");
            if let Some(b) = boundaries {
                collect_from_boundaries(b, geom_type, &mut indices);
            }
        }
    }
    indices
}

fn collect_from_boundaries(val: &Value, geom_type: &str, indices: &mut HashSet<usize>) {
    match geom_type {
        "MultiPoint" | "GeometryInstance" => {
            if let Some(arr) = val.as_array() {
                for item in arr {
                    if let Some(n) = item.as_i64() {
                        indices.insert(n as usize);
                    }
                }
            }
        }
        "MultiLineString" => {
            if let Some(arr) = val.as_array() {
                for line in arr {
                    collect_ints_from_array(line, indices);
                }
            }
        }
        "MultiSurface" | "CompositeSurface" => {
            if let Some(arr) = val.as_array() {
                for surface in arr {
                    collect_from_surface(surface, indices);
                }
            }
        }
        "Solid" => {
            if let Some(arr) = val.as_array() {
                for shell in arr {
                    if let Some(shell_arr) = shell.as_array() {
                        for surface in shell_arr {
                            collect_from_surface(surface, indices);
                        }
                    }
                }
            }
        }
        "MultiSolid" | "CompositeSolid" => {
            if let Some(arr) = val.as_array() {
                for solid in arr {
                    if let Some(solid_arr) = solid.as_array() {
                        for shell in solid_arr {
                            if let Some(shell_arr) = shell.as_array() {
                                for surface in shell_arr {
                                    collect_from_surface(surface, indices);
                                }
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn collect_from_surface(val: &Value, indices: &mut HashSet<usize>) {
    if let Some(rings) = val.as_array() {
        for ring in rings {
            collect_ints_from_array(ring, indices);
        }
    }
}

fn collect_ints_from_array(val: &Value, indices: &mut HashSet<usize>) {
    if let Some(arr) = val.as_array() {
        for item in arr {
            if let Some(n) = item.as_i64() {
                indices.insert(n as usize);
            }
        }
    }
}

pub fn get_all_city_objects(
    doc: &CityJsonDocument,
) -> Vec<(String, Value)> {
    let mut objects = Vec::new();

    if let Some(objs) = doc
        .header
        .get("CityObjects")
        .and_then(|v| v.as_object())
    {
        for (k, v) in objs {
            objects.push((k.clone(), v.clone()));
        }
    }

    for feature in &doc.features {
        if let Some(objs) = feature.get("CityObjects").and_then(|v| v.as_object()) {
            for (k, v) in objs {
                objects.push((k.clone(), v.clone()));
            }
        }
    }

    objects
}

pub fn get_all_city_objects_mut(
    doc: &mut CityJsonDocument,
) -> Vec<(String, &mut Value)> {
    let mut objects = Vec::new();

    if let Some(objs) = doc
        .header
        .get_mut("CityObjects")
        .and_then(|v| v.as_object_mut())
    {
        for (k, v) in objs.iter_mut() {
            objects.push((k.clone(), v));
        }
    }

    for feature in &mut doc.features {
        if let Some(objs) = feature.get_mut("CityObjects").and_then(|v| v.as_object_mut()) {
            for (k, v) in objs.iter_mut() {
                objects.push((k.clone(), v));
            }
        }
    }

    objects
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_cityjson() {
        let doc = read_file("data/3dbag_b2.city.json").unwrap();
        assert_eq!(doc.original_format, InputFormat::CityJSON);
        assert!(doc.features.is_empty());
        let objs = get_all_city_objects(&doc);
        assert!(!objs.is_empty(), "Should have city objects");
        assert_eq!(
            doc.header.get("type").and_then(|v| v.as_str()),
            Some("CityJSON")
        );
        assert_eq!(
            doc.header.get("version").and_then(|v| v.as_str()),
            Some("2.0")
        );
    }

    #[test]
    fn test_read_cityjsonseq() {
        let doc = read_file("data/3dbag_b2.city.jsonl").unwrap();
        assert_eq!(doc.original_format, InputFormat::CityJSONSeq);
        assert!(!doc.features.is_empty(), "Should have features");
        for f in &doc.features {
            assert_eq!(
                f.get("type").and_then(|v| v.as_str()),
                Some("CityJSONFeature")
            );
        }
        let objs = get_all_city_objects(&doc);
        assert!(!objs.is_empty(), "Should find objects in features");
    }

    #[test]
    fn test_collapse() {
        let doc = read_file("data/3dbag_b2.city.jsonl").unwrap();
        let collapsed = collapse(&doc);
        let obj = collapsed.as_object().unwrap();
        assert_eq!(
            obj.get("type").and_then(|v| v.as_str()),
            Some("CityJSON")
        );
        let objects = obj.get("CityObjects").and_then(|v| v.as_object());
        assert!(objects.is_some());
        assert!(!objects.unwrap().is_empty());
    }

    #[test]
    fn test_roundtrip() {
        let doc = read_file("data/3dbag_b2.city.json").unwrap();
        let collapsed = collapse(&doc);
        let json = serde_json::to_string(&collapsed).unwrap();
        let reparsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            reparsed.get("type").and_then(|v| v.as_str()),
            Some("CityJSON")
        );
        let objs = reparsed.get("CityObjects").and_then(|v| v.as_object());
        assert!(objs.is_some());
        assert_eq!(objs.unwrap().len(), get_all_city_objects(&doc).len());
    }
}
