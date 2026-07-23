use mrio_core::io;
use mrio_core::model::{CityJsonDocument, OutputFormat};
use mrio_core::ops;
use mrio_core::stats;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[wasm_bindgen]
pub struct WasmDocument {
    doc: CityJsonDocument,
    filename: String,
}

#[derive(Serialize, Deserialize)]
pub struct StatsResult {
    pub format_name: String,
    pub version: String,
    pub total_objects: usize,
    pub objects_with_attrs: usize,
    pub total_vertices: usize,
    pub object_type_counts: Vec<(String, usize)>,
    pub other_object_types: Vec<(String, usize)>,
    pub attribute_inventory: Vec<(String, usize, String)>,
    pub extensions: Vec<(String, String)>,
    pub crs: String,
}

#[derive(Serialize, Deserialize)]
pub struct OpResult {
    pub summary: String,
    pub affected: usize,
    pub is_error: bool,
}

#[wasm_bindgen]
impl WasmDocument {
    #[wasm_bindgen(constructor)]
    pub fn new(content: &str, filename: &str) -> Result<WasmDocument, JsValue> {
        let doc = io::parse(content, filename).map_err(|e| JsValue::from_str(&e))?;
        Ok(WasmDocument {
            doc,
            filename: filename.to_string(),
        })
    }

    pub fn get_stats(&self) -> Result<JsValue, JsValue> {
        let s = stats::compute_stats(&self.doc);
        let result = StatsResult {
            format_name: s.format_name,
            version: s.version,
            total_objects: s.total_objects,
            objects_with_attrs: s.objects_with_attrs,
            total_vertices: s.total_vertices,
            object_type_counts: s.object_type_counts.into_iter().collect(),
            other_object_types: s.other_object_types.into_iter().collect(),
            attribute_inventory: s.attribute_inventory,
            extensions: s.extensions,
            crs: s.crs,
        };
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn get_attributes(&self) -> Result<JsValue, JsValue> {
        let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for (_id, obj) in io::get_all_city_objects(&self.doc) {
            if let Some(attrs) = obj.get("attributes").and_then(|v| v.as_object()) {
                for key in attrs.keys() {
                    names.insert(key.clone());
                }
            }
        }
        let attrs: Vec<String> = names.into_iter().collect();
        serde_wasm_bindgen::to_value(&attrs).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn run_operation(&mut self, op: &str, param: &str) -> Result<JsValue, JsValue> {
        let report = match op {
            "add_roof_area" => ops::add_roof_area(&mut self.doc),
            "add_volume" => ops::add_volume(&mut self.doc),
            "remove_attribute" => ops::remove_attribute(&mut self.doc, param),
            "rename_attribute" => {
                let parts: Vec<&str> = param.splitn(2, '|').collect();
                if parts.len() != 2 {
                    return Err(JsValue::from_str("Expected 'old_name|new_name'"));
                }
                ops::rename_attribute(&mut self.doc, parts[0], parts[1])
            }
            "add_from_csv" => ops::add_attributes_from_csv(&mut self.doc, param),
            "set_epsg" => ops::set_crs(&mut self.doc, param),
            "roofer2multiroofs" => ops::roofer2multiroofs(&mut self.doc),
            "validate_schema" => ops::validate_schema(&self.doc),
            _ => return Err(JsValue::from_str(&format!("Unknown operation: {}", op))),
        };

        let result = OpResult {
            summary: report.summary,
            affected: report.affected,
            is_error: report.is_error,
        };
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn serialize(&self, format: &str) -> Result<String, JsValue> {
        let fmt = match format {
            "cityjson" => OutputFormat::CityJSON,
            "cityjsonseq" => OutputFormat::CityJSONSeq,
            _ => return Err(JsValue::from_str(&format!("Unknown format: {}", format))),
        };
        io::serialize(&self.doc, fmt).map_err(|e| JsValue::from_str(&e))
    }

    pub fn get_filename(&self) -> String {
        self.filename.clone()
    }

    pub fn get_output_format(&self) -> String {
        let fmt: OutputFormat = self.doc.original_format.into();
        match fmt {
            OutputFormat::CityJSON => "cityjson".to_string(),
            OutputFormat::CityJSONSeq => "cityjsonseq".to_string(),
        }
    }

    pub fn get_extension_urls(&self) -> JsValue {
        if let Some(exts) = self
            .doc
            .header
            .get("extensions")
            .and_then(|v| v.as_object())
        {
            let urls: Vec<String> = exts
                .iter()
                .filter_map(|(name, ext)| {
                    ext.get("url")
                        .and_then(|u| u.as_str())
                        .map(|url| format!("{}|{}", name, url))
                })
                .collect();
            if urls.is_empty() {
                JsValue::NULL
            } else {
                JsValue::from_str(&urls.join("\n"))
            }
        } else {
            JsValue::NULL
        }
    }

    pub fn validate_with_extensions(&self, extension_schemas: &str) -> Result<JsValue, JsValue> {
        let report = ops::validate_schema_with_extensions(&self.doc, extension_schemas);
        let result = OpResult {
            summary: report.summary,
            affected: report.affected,
            is_error: report.is_error,
        };
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}
