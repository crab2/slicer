use serde::{Deserialize, Serialize};

pub const PDF_STRUCTURE_PARSER_NAME: &str = "opendataloader-pdf";
pub const PDF_STRUCTURE_PARSER_VERSION: &str = "2.5.0";
pub const PDF_STRUCTURE_SCHEMA_VERSION: &str = "opendataloader_pdf_json_v2";
pub const PDF_STRUCTURE_OPTIONS_JSON: &str = r#"{"format":"json,markdown,html,pdf","hybrid":"off","image_output":"external","image_format":"png","reading_order":"xycut","threads":1}"#;
pub const VISUAL_MODULE_ANALYSIS_SCHEMA_VERSION: &str = "visual_module_analysis_v1";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualModuleAnalysisModelInfo {
    pub provider: String,
    pub model_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisualModuleAnalysisV1 {
    pub schema_version: String,
    pub block_id: String,
    pub description: String,
    pub visible_text: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub model: VisualModuleAnalysisModelInfo,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VisualModuleCounts {
    pub total: i64,
    pub pending: i64,
    pub succeeded: i64,
    pub failed: i64,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub struct NormalizedBbox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl NormalizedBbox {
    pub fn is_valid(self) -> bool {
        [self.x, self.y, self.width, self.height]
            .into_iter()
            .all(f64::is_finite)
            && self.x >= 0.0
            && self.y >= 0.0
            && self.width > 0.0
            && self.height > 0.0
            && self.x + self.width <= 1.000_001
            && self.y + self.height <= 1.000_001
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub struct PdfPageGeometry {
    pub width_points: f64,
    pub height_points: f64,
    pub crop_left_points: f64,
    pub crop_bottom_points: f64,
    pub crop_right_points: f64,
    pub crop_top_points: f64,
    pub rotation_degrees: i64,
}

impl PdfPageGeometry {
    pub fn unrotated_width(self) -> f64 {
        self.crop_right_points - self.crop_left_points
    }

    pub fn unrotated_height(self) -> f64 {
        self.crop_top_points - self.crop_bottom_points
    }

    pub fn is_valid(self) -> bool {
        [
            self.width_points,
            self.height_points,
            self.crop_left_points,
            self.crop_bottom_points,
            self.crop_right_points,
            self.crop_top_points,
        ]
        .into_iter()
        .all(f64::is_finite)
            && self.width_points > 0.0
            && self.height_points > 0.0
            && self.unrotated_width() > 0.0
            && self.unrotated_height() > 0.0
            && matches!(self.rotation_degrees, 0 | 90 | 180 | 270)
    }
}

pub fn normalize_pdf_bbox(raw: [f64; 4], geometry: PdfPageGeometry) -> Option<NormalizedBbox> {
    if !geometry.is_valid() || !raw.into_iter().all(f64::is_finite) {
        return None;
    }

    let left = raw[0]
        .min(raw[2])
        .clamp(geometry.crop_left_points, geometry.crop_right_points)
        - geometry.crop_left_points;
    let right = raw[0]
        .max(raw[2])
        .clamp(geometry.crop_left_points, geometry.crop_right_points)
        - geometry.crop_left_points;
    let bottom = raw[1]
        .min(raw[3])
        .clamp(geometry.crop_bottom_points, geometry.crop_top_points)
        - geometry.crop_bottom_points;
    let top = raw[1]
        .max(raw[3])
        .clamp(geometry.crop_bottom_points, geometry.crop_top_points)
        - geometry.crop_bottom_points;

    if right <= left || top <= bottom {
        return None;
    }

    let page_width = geometry.unrotated_width();
    let page_height = geometry.unrotated_height();
    let corners = [(left, bottom), (left, top), (right, bottom), (right, top)];
    let (display_width, display_height) = match geometry.rotation_degrees {
        0 | 180 => (page_width, page_height),
        90 | 270 => (page_height, page_width),
        _ => return None,
    };

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (x, y) in corners {
        let (display_x, display_y) = match geometry.rotation_degrees {
            0 => (x, page_height - y),
            90 => (y, x),
            180 => (page_width - x, y),
            270 => (page_height - y, page_width - x),
            _ => return None,
        };
        min_x = min_x.min(display_x);
        min_y = min_y.min(display_y);
        max_x = max_x.max(display_x);
        max_y = max_y.max(display_y);
    }

    let normalized = NormalizedBbox {
        x: (min_x / display_width).clamp(0.0, 1.0),
        y: (min_y / display_height).clamp(0.0, 1.0),
        width: ((max_x - min_x) / display_width).clamp(0.0, 1.0),
        height: ((max_y - min_y) / display_height).clamp(0.0, 1.0),
    };
    normalized.is_valid().then_some(normalized)
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PdfParseRun {
    pub parse_id: String,
    pub document_id: String,
    pub parser_name: String,
    pub parser_version: String,
    pub schema_version: String,
    pub parser_options_json: String,
    pub raw_json_path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct DocumentArtifactInput {
    pub artifact_id: String,
    pub document_id: String,
    pub kind: String,
    pub relative_path: String,
    pub content_hash: String,
    pub parser_name: Option<String>,
    pub parser_version: Option<String>,
    pub parser_options_json: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PdfStructurePage {
    pub page_id: String,
    pub page_number: i64,
    pub geometry: PdfPageGeometry,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PdfContentBlockDto {
    pub block_id: String,
    pub parse_id: String,
    pub document_id: String,
    pub page_id: String,
    pub page_number: i64,
    pub parent_block_id: Option<String>,
    pub source_element_id: Option<String>,
    pub ordinal: i64,
    pub block_type: String,
    pub source_text: String,
    pub enrichment_json: Option<String>,
    pub raw_json: String,
    pub source_image_path: Option<String>,
    pub is_indexable: bool,
    pub is_visual: bool,
    pub is_decorative: bool,
    pub bbox: Option<NormalizedBbox>,
}

impl PdfContentBlockDto {
    pub fn index_text(&self) -> String {
        let mut parts = Vec::new();
        if !self.source_text.trim().is_empty() {
            parts.push(self.source_text.trim().to_string());
        }
        if let Some(enrichment) = self.enrichment_json.as_deref() {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(enrichment) {
                for key in ["description", "summary", "visible_text", "caption"] {
                    if let Some(text) = value.get(key).and_then(serde_json::Value::as_str) {
                        if !text.trim().is_empty() {
                            parts.push(text.trim().to_string());
                        }
                    }
                }
                if let Some(keywords) = value.get("keywords").and_then(serde_json::Value::as_array)
                {
                    for keyword in keywords.iter().filter_map(serde_json::Value::as_str) {
                        if !keyword.trim().is_empty() {
                            parts.push(keyword.trim().to_string());
                        }
                    }
                }
            }
        }
        parts.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_pdf_bbox, PdfPageGeometry};

    fn geometry(rotation_degrees: i64) -> PdfPageGeometry {
        PdfPageGeometry {
            width_points: 120.0,
            height_points: 240.0,
            crop_left_points: 10.0,
            crop_bottom_points: 20.0,
            crop_right_points: 110.0,
            crop_top_points: 220.0,
            rotation_degrees,
        }
    }

    fn assert_bbox(actual: super::NormalizedBbox, expected: [f64; 4]) {
        let actual = [actual.x, actual.y, actual.width, actual.height];
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
        }
    }

    #[test]
    fn normalizes_crop_box_and_all_page_rotations() {
        let raw = [10.0, 20.0, 60.0, 120.0];
        assert_bbox(
            normalize_pdf_bbox(raw, geometry(0)).expect("0 degrees"),
            [0.0, 0.5, 0.5, 0.5],
        );
        assert_bbox(
            normalize_pdf_bbox(raw, geometry(90)).expect("90 degrees"),
            [0.0, 0.0, 0.5, 0.5],
        );
        assert_bbox(
            normalize_pdf_bbox(raw, geometry(180)).expect("180 degrees"),
            [0.5, 0.0, 0.5, 0.5],
        );
        assert_bbox(
            normalize_pdf_bbox(raw, geometry(270)).expect("270 degrees"),
            [0.5, 0.5, 0.5, 0.5],
        );
    }

    #[test]
    fn preserves_asymmetric_box_orientation_for_all_page_rotations() {
        let raw = [20.0, 50.0, 70.0, 90.0];
        assert_bbox(
            normalize_pdf_bbox(raw, geometry(0)).expect("0 degrees"),
            [0.1, 0.65, 0.5, 0.2],
        );
        assert_bbox(
            normalize_pdf_bbox(raw, geometry(90)).expect("90 degrees"),
            [0.15, 0.1, 0.2, 0.5],
        );
        assert_bbox(
            normalize_pdf_bbox(raw, geometry(180)).expect("180 degrees"),
            [0.4, 0.15, 0.5, 0.2],
        );
        assert_bbox(
            normalize_pdf_bbox(raw, geometry(270)).expect("270 degrees"),
            [0.65, 0.4, 0.2, 0.5],
        );
    }

    #[test]
    fn clamps_partial_boxes_and_rejects_empty_or_invalid_boxes() {
        assert_bbox(
            normalize_pdf_bbox([-50.0, -50.0, 60.0, 120.0], geometry(0)).expect("clamped box"),
            [0.0, 0.5, 0.5, 0.5],
        );
        assert!(normalize_pdf_bbox([0.0, 0.0, 5.0, 5.0], geometry(0)).is_none());
        assert!(normalize_pdf_bbox([f64::NAN, 0.0, 1.0, 1.0], geometry(0)).is_none());
    }
}
