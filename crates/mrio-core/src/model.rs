use serde_json::{Map, Value};

#[derive(Debug, Clone)]
pub struct CityJsonDocument {
    pub header: Map<String, Value>,
    pub features: Vec<Map<String, Value>>,
    pub original_format: InputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputFormat {
    CityJSON,
    CityJSONSeq,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    CityJSON,
    CityJSONSeq,
}

impl InputFormat {
    pub fn from_path(path: &str) -> Self {
        if path.ends_with(".jsonl") {
            InputFormat::CityJSONSeq
        } else {
            InputFormat::CityJSON
        }
    }
}

impl OutputFormat {
    pub fn label(&self) -> &str {
        match self {
            OutputFormat::CityJSON => "CityJSON",
            OutputFormat::CityJSONSeq => "CityJSONSeq",
        }
    }

    pub fn extension(&self) -> &str {
        match self {
            OutputFormat::CityJSON => "city.json",
            OutputFormat::CityJSONSeq => "city.jsonl",
        }
    }
}

impl From<InputFormat> for OutputFormat {
    fn from(f: InputFormat) -> Self {
        match f {
            InputFormat::CityJSON => OutputFormat::CityJSON,
            InputFormat::CityJSONSeq => OutputFormat::CityJSONSeq,
        }
    }
}

pub const CITY_OBJECT_TYPES: &[&str] = &[
    "Building",
    "BuildingPart",
    "BuildingInstallation",
    "BuildingConstructiveElement",
    "BuildingFurniture",
    "BuildingStorey",
    "BuildingRoom",
    "BuildingUnit",
    "Bridge",
    "BridgePart",
    "BridgeInstallation",
    "BridgeConstructiveElement",
    "BridgeRoom",
    "BridgeFurniture",
    "CityFurniture",
    "CityObjectGroup",
    "GenericCityObject",
    "LandUse",
    "OtherConstruction",
    "PlantCover",
    "SolitaryVegetationObject",
    "TINRelief",
    "Road",
    "Railway",
    "Waterway",
    "TransportSquare",
    "Tunnel",
    "TunnelPart",
    "TunnelInstallation",
    "TunnelConstructiveElement",
    "TunnelHollowSpace",
    "TunnelFurniture",
    "WaterBody",
];
