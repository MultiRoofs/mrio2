use std::collections::HashMap;
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

pub fn add_attributes_from_csv(doc: &mut CityJsonDocument, csv_content: &str) -> OpReport {
    let first_line = csv_content.lines().next().unwrap_or("");
    let delim = if first_line.matches(';').count() > first_line.matches(',').count() {
        b';'
    } else {
        b','
    };

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .from_reader(csv_content.as_bytes());

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
    let mut id_map: HashMap<String, usize> = HashMap::new();
    let objects = io::get_all_city_objects_mut(doc);
    for (i, (id, _obj)) in objects.iter().enumerate() {
        id_map.insert(id.clone(), i);
    }
    drop(objects);

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
        "Added attributes to {} object(s) from CSV",
        updated_count,
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
    if !doc.features.is_empty() {
        let collapsed = io::collapse(doc);
        doc.header = collapsed.as_object().cloned().unwrap_or_default();
        doc.features.clear();
    }

    let scale: [f64; 3] = doc
        .header
        .get("transform")
        .and_then(|t| t.as_object())
        .and_then(|t| t.get("scale"))
        .and_then(|v| v.as_array())
        .map(|a| {
            let s0 = a.first().and_then(|v| v.as_f64()).unwrap_or(1.0);
            let s1 = a.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0);
            let s2 = a.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0);
            [s0, s1, s2]
        })
        .unwrap_or([1.0, 1.0, 1.0]);
    let translate: [f64; 3] = doc
        .header
        .get("transform")
        .and_then(|t| t.as_object())
        .and_then(|t| t.get("translate"))
        .and_then(|v| v.as_array())
        .map(|a| {
            let t0 = a.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
            let t1 = a.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let t2 = a.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0);
            [t0, t1, t2]
        })
        .unwrap_or([0.0, 0.0, 0.0]);
    let vertices_arr: Vec<Value> = doc
        .header
        .get("vertices")
        .and_then(|v| v.as_array())
        .map(|a| a.clone())
        .unwrap_or_default();

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

    for id in &part_ids {
        city_objects.remove(id);
    }

    for (_id, obj) in city_objects.iter_mut() {
        let roof_area = compute_roof_area(obj, &vertices_arr, &scale, &translate);
        if roof_area > 0.0 {
            let attrs = obj
                .get_mut("attributes")
                .and_then(|v| v.as_object_mut());
            if let Some(attrs) = attrs {
                attrs.insert(
                    "+roof-total-area".to_string(),
                    serde_json::json!((roof_area * 1000.0).round() / 1000.0),
                );
            } else {
                let mut new_attrs = Map::new();
                new_attrs.insert(
                    "+roof-total-area".to_string(),
                    serde_json::json!((roof_area * 1000.0).round() / 1000.0),
                );
                obj.as_object_mut()
                    .map(|m| m.insert("attributes".to_string(), Value::Object(new_attrs)));
            }
        }
    }

    let mut rename_count = 0;
    for (_id, obj) in city_objects.iter_mut() {
        if let Some(attrs) = obj.get_mut("attributes").and_then(|v| v.as_object_mut()) {
            if let Some(val) = attrs.remove("b3_volume") {
                attrs.insert("+building-volume".to_string(), val);
                rename_count += 1;
            }
        }
    }

    let ext_name = "multiroofs";
    let ext_value = serde_json::json!({
        "url": "https://raw.githubusercontent.com/MultiRoofs/cityjson-extension/refs/heads/main/multiroofs.ext.json",
        "version": "0.2.0"
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
        "Roofer→MultiRoofs: merged {} BuildingPart(s), removed lod=0 geometry, renamed {} attribute(s), added +roof-total-area, added extension",
        part_ids.len(),
        rename_count,
    );

    OpReport {
        summary,
        affected: part_ids.len(),
        is_error: part_ids.is_empty(),
    }
}

fn compute_roof_area(obj: &Value, vertices: &[Value], scale: &[f64; 3], translate: &[f64; 3]) -> f64 {
    let geoms = match obj.get("geometry").and_then(|v| v.as_array()) {
        Some(g) => g,
        None => return 0.0,
    };
    let mut total = 0.0;
    for geom in geoms {
        let geom_type = geom.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let semantics = match geom.get("semantics").and_then(|v| v.as_object()) {
            Some(s) => s,
            None => continue,
        };
        let surfaces = match semantics.get("surfaces").and_then(|v| v.as_array()) {
            Some(s) => s,
            None => continue,
        };
        let values = match semantics.get("values").and_then(|v| v.as_array()) {
            Some(v) => v,
            None => continue,
        };
        let boundaries = match geom.get("boundaries").and_then(|v| v.as_array()) {
            Some(b) => b,
            None => continue,
        };

        let roof_surface_indices: Vec<usize> = surfaces
            .iter()
            .enumerate()
            .filter(|(_, s)| s.get("type").and_then(|t| t.as_str()) == Some("RoofSurface"))
            .map(|(i, _)| i)
            .collect();

        if roof_surface_indices.is_empty() {
            continue;
        }

        match geom_type {
            "Solid" | "MultiSurface" | "CompositeSurface" => {
                let num_shells = if geom_type == "Solid" {
                    boundaries.len()
                } else {
                    1
                };

                for shell_idx in 0..num_shells {
                    let faces = if geom_type == "Solid" {
                        match boundaries[shell_idx].as_array() {
                            Some(f) => f.clone(),
                            None => continue,
                        }
                    } else {
                        let all_faces: Vec<Value> = boundaries.iter().cloned().collect();
                        all_faces
                    };

                    let shell_values: Vec<Value> = if geom_type == "Solid" {
                        match values.get(shell_idx).and_then(|v| v.as_array()) {
                            Some(arr) => arr.to_vec(),
                            None => vec![],
                        }
                    } else {
                        values.to_vec()
                    };

                    for (face_idx, face) in faces.iter().enumerate() {
                        let sem_idx = match shell_values.get(face_idx) {
                            Some(v) => v.as_i64().unwrap_or(-1) as usize,
                            None => continue,
                        };
                        if !roof_surface_indices.contains(&sem_idx) {
                            continue;
                        }

                        let rings = match face.as_array() {
                            Some(r) => r,
                            None => continue,
                        };
                        if rings.is_empty() {
                            continue;
                        }

                        let outer_indices: Vec<usize> = match rings[0].as_array() {
                            Some(arr) => arr.iter().filter_map(|v| v.as_i64().map(|n| n as usize)).collect(),
                            None => continue,
                        };
                        let outer_area = ring_area_3d(&outer_indices, vertices, scale, translate);

                        let mut inner_area = 0.0;
                        for ring_idx in 1..rings.len() {
                            if let Some(arr) = rings[ring_idx].as_array() {
                                let indices: Vec<usize> = arr.iter().filter_map(|v| v.as_i64().map(|n| n as usize)).collect();
                                inner_area += ring_area_3d(&indices, vertices, scale, translate);
                            }
                        }

                        total += (outer_area - inner_area).abs();
                    }
                }
            }
            _ => {}
        }
    }
    total
}

fn ring_area_3d(indices: &[usize], vertices: &[Value], scale: &[f64; 3], translate: &[f64; 3]) -> f64 {
    if indices.len() < 3 {
        return 0.0;
    }
    let mut pts: Vec<[f64; 3]> = Vec::with_capacity(indices.len());
    for &idx in indices {
        if let Some(v) = vertices.get(idx).and_then(|v| v.as_array()) {
            let x = v.get(0).and_then(|n| n.as_f64()).unwrap_or(0.0) * scale[0] + translate[0];
            let y = v.get(1).and_then(|n| n.as_f64()).unwrap_or(0.0) * scale[1] + translate[1];
            let z = v.get(2).and_then(|n| n.as_f64()).unwrap_or(0.0) * scale[2] + translate[2];
            pts.push([x, y, z]);
        }
    }
    if pts.len() < 3 {
        return 0.0;
    }
    let mut cx = 0.0;
    let mut cy = 0.0;
    let mut cz = 0.0;
    for i in 0..pts.len() {
        let j = (i + 1) % pts.len();
        cx += pts[i][1] * pts[j][2] - pts[i][2] * pts[j][1];
        cy += pts[i][2] * pts[j][0] - pts[i][0] * pts[j][2];
        cz += pts[i][0] * pts[j][1] - pts[i][1] * pts[j][0];
    }
    0.5 * (cx * cx + cy * cy + cz * cz).sqrt()
}

fn ring_volume_contribution(indices: &[usize], vertices: &[Value], scale: &[f64; 3], translate: &[f64; 3]) -> f64 {
    if indices.len() < 3 {
        return 0.0;
    }
    let n = if indices.len() > 1 && indices[0] == indices[indices.len() - 1] {
        indices.len() - 1
    } else {
        indices.len()
    };
    if n < 3 {
        return 0.0;
    }
    let get_pt = |idx: usize| -> Option<[f64; 3]> {
        let v = vertices.get(idx).and_then(|v| v.as_array())?;
        Some([
            v.get(0).and_then(|n| n.as_f64()).unwrap_or(0.0) * scale[0] + translate[0],
            v.get(1).and_then(|n| n.as_f64()).unwrap_or(0.0) * scale[1] + translate[1],
            v.get(2).and_then(|n| n.as_f64()).unwrap_or(0.0) * scale[2] + translate[2],
        ])
    };
    let p0 = match get_pt(indices[0]) {
        Some(p) => p,
        None => return 0.0,
    };
    let mut vol = 0.0;
    for i in 1..n - 1 {
        let b = match get_pt(indices[i]) {
            Some(p) => p,
            None => continue,
        };
        let c = match get_pt(indices[i + 1]) {
            Some(p) => p,
            None => continue,
        };
        // Use p0 as reference point for the tetrahedron.
        // u = b - p0 and w = c - p0 have small magnitude (span a single polygon),
        // so the cross product (u × w) involves much smaller numbers than
        // the origin-based formula (v1 × v2) which would span the entire mesh.
        let ux = b[0] - p0[0];
        let uy = b[1] - p0[1];
        let uz = b[2] - p0[2];
        let wx = c[0] - p0[0];
        let wy = c[1] - p0[1];
        let wz = c[2] - p0[2];
        // Scalar triple product: p0 · (u × w)
        let det = p0[0] * (uy * wz - uz * wy)
                + p0[1] * (uz * wx - ux * wz)
                + p0[2] * (ux * wy - uy * wx);
        vol += det;
    }
    vol / 6.0
}

fn compute_volume(obj: &Value, vertices: &[Value], scale: &[f64; 3], translate: &[f64; 3]) -> f64 {
    let geoms = match obj.get("geometry").and_then(|v| v.as_array()) {
        Some(g) => g,
        None => return 0.0,
    };
    let mut total = 0.0;
    for geom in geoms {
        let geom_type = geom.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let boundaries = match geom.get("boundaries").and_then(|v| v.as_array()) {
            Some(b) => b,
            None => continue,
        };
        match geom_type {
            "Solid" => {
                for shell in boundaries {
                    let faces = match shell.as_array() {
                        Some(f) => f,
                        None => continue,
                    };
                    for face in faces {
                        let rings = match face.as_array() {
                            Some(r) => r,
                            None => continue,
                        };
                        for ring in rings {
                            if let Some(arr) = ring.as_array() {
                                let indices: Vec<usize> =
                                    arr.iter().filter_map(|v| v.as_i64().map(|n| n as usize)).collect();
                                total += ring_volume_contribution(&indices, vertices, scale, translate);
                            }
                        }
                    }
                }
            }
            "CompositeSurface" => {
                for face in boundaries {
                    let rings = match face.as_array() {
                        Some(r) => r,
                        None => continue,
                    };
                    for ring in rings {
                        if let Some(arr) = ring.as_array() {
                            let indices: Vec<usize> =
                                arr.iter().filter_map(|v| v.as_i64().map(|n| n as usize)).collect();
                            total += ring_volume_contribution(&indices, vertices, scale, translate);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    total.abs()
}

pub fn add_volume(doc: &mut CityJsonDocument) -> OpReport {
    if !doc.features.is_empty() {
        let collapsed = io::collapse(doc);
        doc.header = collapsed.as_object().cloned().unwrap_or_default();
        doc.features.clear();
    }

    let scale: [f64; 3] = doc
        .header
        .get("transform")
        .and_then(|t| t.as_object())
        .and_then(|t| t.get("scale"))
        .and_then(|v| v.as_array())
        .map(|a| {
            [
                a.first().and_then(|v| v.as_f64()).unwrap_or(1.0),
                a.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0),
                a.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0),
            ]
        })
        .unwrap_or([1.0, 1.0, 1.0]);
    let translate: [f64; 3] = doc
        .header
        .get("transform")
        .and_then(|t| t.as_object())
        .and_then(|t| t.get("translate"))
        .and_then(|v| v.as_array())
        .map(|a| {
            [
                a.first().and_then(|v| v.as_f64()).unwrap_or(0.0),
                a.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0),
                a.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0),
            ]
        })
        .unwrap_or([0.0, 0.0, 0.0]);
    let vertices: Vec<Value> = doc
        .header
        .get("vertices")
        .and_then(|v| v.as_array())
        .map(|a| a.clone())
        .unwrap_or_default();

    let city_objects = doc.header.get_mut("CityObjects").and_then(|v| v.as_object_mut());

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

    let mut count = 0;
    for (_id, obj) in city_objects.iter_mut() {
        let volume = compute_volume(obj, &vertices, &scale, &translate);
        if volume > 0.0 {
            let attrs = obj.get_mut("attributes").and_then(|v| v.as_object_mut());
            if let Some(attrs) = attrs {
                attrs.insert(
                    "+building-volume".to_string(),
                    serde_json::json!((volume * 1000.0).round() / 1000.0),
                );
            } else {
                let mut new_attrs = Map::new();
                new_attrs.insert(
                    "+building-volume".to_string(),
                    serde_json::json!((volume * 1000.0).round() / 1000.0),
                );
                obj.as_object_mut()
                    .map(|m| m.insert("attributes".to_string(), Value::Object(new_attrs)));
            }
            count += 1;
        }
    }

    OpReport {
        summary: format!("Added +building-volume to {} object(s)", count),
        affected: count,
        is_error: count == 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io;

    #[test]
    fn test_roofer2multiroofs() {
        let mut doc = io::read_file("../../data/roofer_output_b2.city.json").unwrap();
        let report = roofer2multiroofs(&mut doc);
        assert!(!report.is_error, "Operation failed: {}", report.summary);
        assert!(report.affected > 0, "No BuildingParts were processed");

        for (id, obj) in io::get_all_city_objects(&doc) {
            let ty = obj.get("type").and_then(|v| v.as_str()).unwrap();
            assert!(
                ty != "BuildingPart",
                "BuildingPart '{}' should have been removed",
                id
            );
        }

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

        let exts = doc.header.get("extensions").and_then(|v| v.as_object());
        assert!(exts.is_some(), "extensions should exist");
        let multiroofs = exts
            .unwrap()
            .get("multiroofs")
            .and_then(|v| v.as_object());
        assert!(multiroofs.is_some(), "multiroofs extension should exist");

        let mut expected_doc = io::read_file("../../data/roofer_corrected_b2.city.json").unwrap();
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

    #[test]
    fn test_add_volume() {
        let mut doc = io::read_file("../../data/3dbag_b2.city.json").unwrap();
        let report = add_volume(&mut doc);
        assert!(!report.is_error, "add_volume failed: {}", report.summary);
        assert!(report.affected > 0, "No objects got volume");

        for (id, obj) in io::get_all_city_objects(&doc) {
            if let Some(attrs) = obj.get("attributes").and_then(|v| v.as_object()) {
                if let Some(vol) = attrs.get("+building-volume").and_then(|v| v.as_f64()) {
                    assert!(vol > 0.0, "Volume for '{}' should be positive, got {}", id, vol);
                    if let Some(reference) = attrs.get("b3_volume_lod22").and_then(|v| v.as_f64()) {
                        let ratio = (vol - reference).abs() / reference;
                        assert!(
                            ratio < 0.05,
                            "Volume for '{}': computed={}, reference={}, ratio={}",
                            id, vol, reference, ratio
                        );
                    }
                }
            }
        }
    }
}

pub fn add_roof_area(doc: &mut CityJsonDocument) -> OpReport {
    if !doc.features.is_empty() {
        let collapsed = io::collapse(doc);
        doc.header = collapsed.as_object().cloned().unwrap_or_default();
        doc.features.clear();
    }

    let scale: [f64; 3] = doc
        .header
        .get("transform")
        .and_then(|t| t.as_object())
        .and_then(|t| t.get("scale"))
        .and_then(|v| v.as_array())
        .map(|a| {
            [
                a.first().and_then(|v| v.as_f64()).unwrap_or(1.0),
                a.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0),
                a.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0),
            ]
        })
        .unwrap_or([1.0, 1.0, 1.0]);
    let translate: [f64; 3] = doc
        .header
        .get("transform")
        .and_then(|t| t.as_object())
        .and_then(|t| t.get("translate"))
        .and_then(|v| v.as_array())
        .map(|a| {
            [
                a.first().and_then(|v| v.as_f64()).unwrap_or(0.0),
                a.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0),
                a.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0),
            ]
        })
        .unwrap_or([0.0, 0.0, 0.0]);
    let vertices: Vec<Value> = doc
        .header
        .get("vertices")
        .and_then(|v| v.as_array())
        .map(|a| a.clone())
        .unwrap_or_default();

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

    let mut count = 0;
    for (_id, obj) in city_objects.iter_mut() {
        let area = compute_roof_area(obj, &vertices, &scale, &translate);
        if area > 0.0 {
            let attrs = obj.get_mut("attributes").and_then(|v| v.as_object_mut());
            if let Some(attrs) = attrs {
                attrs.insert(
                    "+roof-total-area".to_string(),
                    serde_json::json!((area * 1000.0).round() / 1000.0),
                );
            } else {
                let mut new_attrs = Map::new();
                new_attrs.insert(
                    "+roof-total-area".to_string(),
                    serde_json::json!((area * 1000.0).round() / 1000.0),
                );
                obj.as_object_mut()
                    .map(|m| m.insert("attributes".to_string(), Value::Object(new_attrs)));
            }
            count += 1;
        }
    }

    OpReport {
        summary: format!("Added +roof-total-area to {} object(s)", count),
        affected: count,
        is_error: count == 0,
    }
}

pub fn validate_schema(doc: &CityJsonDocument) -> OpReport {
    let collapsed = crate::io::collapse(doc);
    let json_str = serde_json::to_string_pretty(&collapsed).unwrap_or_default();
    #[allow(unused_mut)]
    let mut validator = cjval::CJValidator::from_str(&json_str);
    let mut lines: Vec<String> = Vec::new();
    let mut has_errors = false;

    #[cfg(feature = "native")]
    {
        use std::io::Read;
        if let Some(exts) = doc.header.get("extensions").and_then(|v| v.as_object()) {
            for (_name, ext_info) in exts.iter() {
                if let Some(url) = ext_info.get("url").and_then(|v| v.as_str()) {
                    let resp = ureq::get(url).call();
                    match resp {
                        Ok(response) => {
                            let mut reader = response.into_body().into_reader();
                            let mut body = String::new();
                            let _ = reader.read_to_string(&mut body);
                            match validator.add_one_extension_from_str(&body) {
                                Ok(()) => {}
                                Err(e) => {
                                    lines.push(format!(
                                        " ! extension '{}' schema parse error: {}",
                                        _name, e
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            lines.push(format!(
                                " ! extension '{}' not fetched ({}), skip ext validation",
                                _name, e
                            ));
                        }
                    }
                }
            }
        }
    }

    #[cfg(not(feature = "native"))]
    {
        if let Some(exts) = doc.header.get("extensions").and_then(|v| v.as_object()) {
            if !exts.is_empty() {
                lines.push(" ! extension schemas not fetched (WASM mode), extension validation skipped".to_string());
            }
        }
    }

    let results = validator.validate();

    for (criterion, val_sum) in results.iter() {
        let icon = if val_sum.is_valid() { "✓" } else { "✗" };
        let kind = if val_sum.is_warning() { "warning" } else { "error" };
        lines.push(format!(" {} {} [{}]", icon, criterion, kind));
        if val_sum.has_errors() {
            for err in val_sum.get_errors() {
                lines.push(format!("    {}", err));
            }
            if !val_sum.is_warning() {
                has_errors = true;
            }
        }
    }

    OpReport {
        summary: lines.join("\n"),
        affected: 0,
        is_error: has_errors,
    }
}

pub fn validate_schema_with_extensions(doc: &CityJsonDocument, extension_schemas_json: &str) -> OpReport {
    let collapsed = crate::io::collapse(doc);
    let json_str = serde_json::to_string_pretty(&collapsed).unwrap_or_default();
    let mut validator = cjval::CJValidator::from_str(&json_str);
    let mut lines: Vec<String> = Vec::new();
    let mut has_errors = false;

    if !extension_schemas_json.trim().is_empty() {
        if let Ok(exts) = serde_json::from_str::<Vec<serde_json::Value>>(extension_schemas_json) {
            for ext in exts {
                let name = ext.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
                let schema = ext.get("schema").and_then(|v| v.as_str()).unwrap_or("");
                
                if schema.starts_with("FETCH_ERROR:") {
                    lines.push(format!(" ! extension '{}' schema not fetched: {}", name, &schema[13..]));
                    continue;
                }
                
                match validator.add_one_extension_from_str(schema) {
                    Ok(()) => {
                        lines.push(format!(" ✓ extension '{}' schema loaded", name));
                    }
                    Err(e) => {
                        lines.push(format!(" ! extension '{}' schema parse error: {}", name, e));
                    }
                }
            }
        } else {
            lines.push(" ! failed to parse extension schemas JSON".to_string());
        }
    }

    let results = validator.validate();

    for (criterion, val_sum) in results.iter() {
        let icon = if val_sum.is_valid() { "✓" } else { "✗" };
        let kind = if val_sum.is_warning() { "warning" } else { "error" };
        lines.push(format!(" {} {} [{}]", icon, criterion, kind));
        if val_sum.has_errors() {
            for err in val_sum.get_errors() {
                lines.push(format!("    {}", err));
            }
            if !val_sum.is_warning() {
                has_errors = true;
            }
        }
    }

    OpReport {
        summary: lines.join("\n"),
        affected: 0,
        is_error: has_errors,
    }
}

pub fn set_crs(doc: &mut CityJsonDocument, epsg: &str) -> OpReport {
    let epsg = epsg.trim();
    if epsg.is_empty() || !epsg.chars().all(|c| c.is_ascii_digit()) {
        return OpReport {
            summary: format!("Invalid EPSG code: '{}'", epsg),
            affected: 0,
            is_error: true,
        };
    }
    let url = format!("https://www.opengis.net/def/crs/EPSG/0/{}", epsg);
    let metadata = doc
        .header
        .get_mut("metadata")
        .and_then(|v| v.as_object_mut());
    match metadata {
        Some(m) => {
            m.insert("referenceSystem".to_string(), Value::String(url));
        }
        None => {
            let mut m = Map::new();
            m.insert("referenceSystem".to_string(), Value::String(url));
            doc.header
                .insert("metadata".to_string(), Value::Object(m));
        }
    }
    OpReport {
        summary: format!("CRS set to EPSG:{}", epsg),
        affected: 1,
        is_error: false,
    }
}

fn parse_csv_value(s: &str) -> Value {
    let s = s.trim();
    if s.is_empty() {
        return Value::Null;
    }
    if let Ok(n) = s.parse::<i64>() {
        return Value::Number(serde_json::Number::from(n));
    }
    if let Ok(n) = s.parse::<f64>() {
        if let Some(num) = serde_json::Number::from_f64(n) {
            return Value::Number(num);
        }
    }
    if s.eq_ignore_ascii_case("true") {
        return Value::Bool(true);
    }
    if s.eq_ignore_ascii_case("false") {
        return Value::Bool(false);
    }
    Value::String(s.to_string())
}
