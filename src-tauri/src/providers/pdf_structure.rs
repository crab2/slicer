use crate::artifacts::workspace_layout::is_link_or_reparse_point;
use crate::domain::pdf_structure::{
    normalize_pdf_bbox, PdfContentBlockDto, PdfParseRun, PdfStructurePage,
    PDF_STRUCTURE_OPTIONS_JSON, PDF_STRUCTURE_PARSER_NAME, PDF_STRUCTURE_PARSER_VERSION,
    PDF_STRUCTURE_SCHEMA_VERSION,
};
use crate::errors::{AppError, AppResult};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{Read, Write};
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;
#[cfg(windows)]
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(windows)]
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

pub const OPENDATALOADER_JAR_SHA256: &str =
    "516ce47832a6726e87cb17db77c20174ca8cabbe9a6b56db1418babc7c9ddcba";
const MAX_PROCESS_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_JSON_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TEXT_VIEW_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ANNOTATED_PDF_BYTES: u64 = 512 * 1024 * 1024;
const MAX_CONTENT_BLOCKS: usize = 100_000;
const MAX_BLOCK_TREE_DEPTH: usize = 32;
const MAX_BLOCK_RAW_JSON_TOTAL_BYTES: usize = 128 * 1024 * 1024;
const MAX_BLOCK_TEXT_TOTAL_BYTES: usize = 128 * 1024 * 1024;
const JAVA_MAX_HEAP_ARG: &str = "-Xmx1024m";
const MAX_EXTRACTED_IMAGE_COUNT: usize = 2_000;
const MAX_EXTRACTED_IMAGE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_EXTRACTED_IMAGES_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
const MAX_EXTRACTED_IMAGE_WIDTH: u32 = 16_384;
const MAX_EXTRACTED_IMAGE_HEIGHT: u32 = 16_384;
const MAX_EXTRACTED_IMAGE_PIXELS: u64 = 40_000_000;
const MAX_EXTRACTED_IMAGES_TOTAL_PIXELS: u64 = 1_000_000_000;
const PROCESS_TERMINATION_GRACE: Duration = Duration::from_secs(2);
const PROCESS_OUTPUT_DRAIN_GRACE: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy)]
struct ImageOutputLimits {
    max_count: usize,
    max_file_bytes: u64,
    max_total_bytes: u64,
    max_width: u32,
    max_height: u32,
    max_pixels: u64,
    max_total_pixels: u64,
}

const IMAGE_OUTPUT_LIMITS: ImageOutputLimits = ImageOutputLimits {
    max_count: MAX_EXTRACTED_IMAGE_COUNT,
    max_file_bytes: MAX_EXTRACTED_IMAGE_BYTES,
    max_total_bytes: MAX_EXTRACTED_IMAGES_TOTAL_BYTES,
    max_width: MAX_EXTRACTED_IMAGE_WIDTH,
    max_height: MAX_EXTRACTED_IMAGE_HEIGHT,
    max_pixels: MAX_EXTRACTED_IMAGE_PIXELS,
    max_total_pixels: MAX_EXTRACTED_IMAGES_TOTAL_PIXELS,
};

#[derive(Debug)]
pub struct PdfStructureExtraction {
    pub run: PdfParseRun,
    pub blocks: Vec<PdfContentBlockDto>,
    pub staging_dir: PathBuf,
    pub raw_json_path: PathBuf,
    pub html_path: PathBuf,
    pub markdown_path: PathBuf,
    pub annotated_pdf_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct OpenDataLoaderPdfProvider {
    workspace_root: PathBuf,
    jar_path: PathBuf,
    timeout: Duration,
}

impl OpenDataLoaderPdfProvider {
    pub fn discover(workspace_root: impl AsRef<Path>) -> AppResult<Self> {
        let jar_path = discover_jar_path().ok_or_else(|| {
            AppError::new(
                "opendataloader_jar_missing",
                "未找到 OpenDataLoader PDF 运行资源，请重新安装应用。",
                "pdf_structure_health",
                false,
            )
        })?;
        Ok(Self::new(workspace_root, jar_path))
    }

    pub fn new(workspace_root: impl AsRef<Path>, jar_path: impl AsRef<Path>) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
            jar_path: jar_path.as_ref().to_path_buf(),
            timeout: Duration::from_secs(180),
        }
    }

    #[cfg(test)]
    fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn health_check(&self) -> AppResult<()> {
        if !self.jar_path.is_file() {
            return Err(AppError::new(
                "opendataloader_jar_missing",
                "未找到 OpenDataLoader PDF 运行资源，请重新安装应用。",
                "pdf_structure_health",
                false,
            )
            .with_details(self.jar_path.display().to_string()));
        }

        let actual_hash = sha256_file(&self.jar_path)?;
        if actual_hash != OPENDATALOADER_JAR_SHA256 {
            return Err(AppError::new(
                "opendataloader_jar_hash_mismatch",
                "OpenDataLoader PDF 运行资源校验失败，请重新安装应用。",
                "pdf_structure_health",
                false,
            )
            .with_details(format!("sha256={actual_hash}")));
        }

        let java_version = run_bounded(
            OsStr::new("java"),
            &[OsString::from("-version")],
            Duration::from_secs(10),
            "java_health_check_failed",
        )?;
        require_supported_java_version(&java_version.stdout, &java_version.stderr)?;

        let output = run_bounded(
            OsStr::new("java"),
            &[
                OsString::from(JAVA_MAX_HEAP_ARG),
                OsString::from("-Djava.awt.headless=true"),
                OsString::from("-jar"),
                self.jar_path.as_os_str().to_owned(),
                OsString::from("--export-options"),
            ],
            Duration::from_secs(20),
            "opendataloader_health_check_failed",
        )?;
        if !output.stdout.contains("\"output-dir\"") {
            return Err(AppError::new(
                "opendataloader_health_check_failed",
                "OpenDataLoader PDF 自检未返回预期的 CLI 契约。",
                "pdf_structure_health",
                false,
            )
            .with_details(output.diagnostics()));
        }
        Ok(())
    }

    pub fn extract(
        &self,
        document_id: &str,
        canonical_pdf_path: &Path,
        pages: &[PdfStructurePage],
        staging_dir: &Path,
    ) -> AppResult<PdfStructureExtraction> {
        self.health_check()?;
        let workspace_root =
            canonical_existing_path(&self.workspace_root, "workspace_root_invalid")?;
        let canonical_pdf_path =
            canonical_existing_path(canonical_pdf_path, "canonical_pdf_missing")?;
        ensure_descendant(
            &workspace_root,
            &canonical_pdf_path,
            "canonical_pdf_outside_workspace",
        )?;

        if staging_dir.exists() {
            return Err(AppError::new(
                "pdf_structure_staging_exists",
                "PDF 结构化暂存目录已存在，请重试导入。",
                "pdf_structure_extract",
                true,
            )
            .with_details(staging_dir.display().to_string()));
        }
        let staging_parent = staging_dir.parent().ok_or_else(|| {
            AppError::new(
                "pdf_structure_staging_invalid",
                "PDF 结构化暂存路径无效。",
                "pdf_structure_extract",
                false,
            )
        })?;
        let staging_parent =
            canonical_existing_path(staging_parent, "pdf_structure_staging_invalid")?;
        ensure_descendant(
            &workspace_root,
            &staging_parent,
            "pdf_structure_output_outside_workspace",
        )?;
        fs::create_dir(staging_dir).map_err(|err| {
            AppError::io(
                "pdf_structure_extract",
                "pdf_structure_staging_create_failed",
                err,
            )
        })?;
        let image_dir = staging_dir.join("images");
        fs::create_dir(&image_dir).map_err(|err| {
            AppError::io(
                "pdf_structure_extract",
                "pdf_structure_image_dir_create_failed",
                err,
            )
        })?;

        let args = vec![
            OsString::from(JAVA_MAX_HEAP_ARG),
            OsString::from("-Djava.awt.headless=true"),
            OsString::from("-jar"),
            self.jar_path.as_os_str().to_owned(),
            OsString::from("--format"),
            OsString::from("json,markdown,html,pdf"),
            OsString::from("--image-output"),
            OsString::from("external"),
            OsString::from("--image-format"),
            OsString::from("png"),
            OsString::from("--image-dir"),
            image_dir.as_os_str().to_owned(),
            OsString::from("--output-dir"),
            staging_dir.as_os_str().to_owned(),
            OsString::from("--reading-order"),
            OsString::from("xycut"),
            OsString::from("--threads"),
            OsString::from("1"),
            OsString::from("--hybrid"),
            OsString::from("off"),
            OsString::from("--quiet"),
            canonical_pdf_path.as_os_str().to_owned(),
        ];

        run_bounded(
            OsStr::new("java"),
            &args,
            self.timeout,
            "opendataloader_extract_failed",
        )?;
        validate_image_output_limits(&image_dir, IMAGE_OUTPUT_LIMITS)?;

        let expected_stem = canonical_pdf_path
            .file_stem()
            .and_then(OsStr::to_str)
            .map(str::to_owned)
            .ok_or_else(|| {
                AppError::new(
                    "opendataloader_output_name_invalid",
                    "无法确定 OpenDataLoader PDF 输出文件名。",
                    "pdf_structure_validate",
                    false,
                )
            })?;
        let raw_json_path = staging_dir.join(format!("{expected_stem}.json"));
        let raw_json_path = canonical_existing_path(&raw_json_path, "opendataloader_json_missing")?;
        let staging_canonical =
            canonical_existing_path(staging_dir, "pdf_structure_staging_invalid")?;
        ensure_descendant(
            &staging_canonical,
            &raw_json_path,
            "opendataloader_output_path_invalid",
        )?;
        let metadata = fs::metadata(&raw_json_path).map_err(|err| {
            AppError::io(
                "pdf_structure_validate",
                "opendataloader_json_metadata_failed",
                err,
            )
        })?;
        if metadata.len() > MAX_JSON_BYTES {
            return Err(AppError::new(
                "opendataloader_json_too_large",
                "OpenDataLoader PDF 输出超过安全大小限制。",
                "pdf_structure_validate",
                false,
            )
            .with_details(format!("bytes={}", metadata.len())));
        }
        let html_path = validate_output_artifact(
            &staging_canonical,
            &format!("{expected_stem}.html"),
            "opendataloader_html_missing",
            "opendataloader_html_too_large",
            MAX_TEXT_VIEW_ARTIFACT_BYTES,
        )?;
        let markdown_path = validate_output_artifact(
            &staging_canonical,
            &format!("{expected_stem}.md"),
            "opendataloader_markdown_missing",
            "opendataloader_markdown_too_large",
            MAX_TEXT_VIEW_ARTIFACT_BYTES,
        )?;
        let annotated_pdf_path = validate_output_artifact(
            &staging_canonical,
            &format!("{expected_stem}_annotated.pdf"),
            "opendataloader_annotated_pdf_missing",
            "opendataloader_annotated_pdf_too_large",
            MAX_ANNOTATED_PDF_BYTES,
        )?;
        let raw_json = fs::read(&raw_json_path).map_err(|err| {
            AppError::io(
                "pdf_structure_validate",
                "opendataloader_json_read_failed",
                err,
            )
        })?;
        let parse_id = Uuid::new_v4().to_string();
        let mut blocks = parse_opendataloader_json(
            document_id,
            &parse_id,
            pages,
            &raw_json,
            &staging_canonical,
        )?;
        for block in &mut blocks {
            if let Some(source) = block.source_image_path.as_deref() {
                let source_path = staging_canonical.join(source);
                let source_path =
                    canonical_existing_path(&source_path, "opendataloader_image_missing")?;
                ensure_descendant(
                    &staging_canonical,
                    &source_path,
                    "opendataloader_image_path_invalid",
                )?;
            }
        }

        Ok(PdfStructureExtraction {
            run: PdfParseRun {
                parse_id,
                document_id: document_id.to_string(),
                parser_name: PDF_STRUCTURE_PARSER_NAME.to_string(),
                parser_version: PDF_STRUCTURE_PARSER_VERSION.to_string(),
                schema_version: PDF_STRUCTURE_SCHEMA_VERSION.to_string(),
                parser_options_json: PDF_STRUCTURE_OPTIONS_JSON.to_string(),
                raw_json_path: String::new(),
            },
            blocks,
            staging_dir: staging_canonical,
            raw_json_path,
            html_path,
            markdown_path,
            annotated_pdf_path,
        })
    }
}

fn discover_jar_path() -> Option<PathBuf> {
    let jar_name = format!("opendataloader-pdf-cli-{PDF_STRUCTURE_PARSER_VERSION}.jar");
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("SLICER_OPENDATALOADER_JAR") {
        candidates.push(PathBuf::from(path));
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("opendataloader-pdf")
            .join(&jar_name),
    );
    if let Ok(executable) = std::env::current_exe() {
        if let Some(dir) = executable.parent() {
            candidates.push(
                dir.join("resources")
                    .join("opendataloader-pdf")
                    .join(&jar_name),
            );
            candidates.push(dir.join("opendataloader-pdf").join(&jar_name));
            candidates.push(
                dir.join("..")
                    .join("Resources")
                    .join("opendataloader-pdf")
                    .join(&jar_name),
            );
        }
    }
    candidates.into_iter().find(|candidate| candidate.is_file())
}

pub fn parse_opendataloader_json(
    document_id: &str,
    parse_id: &str,
    pages: &[PdfStructurePage],
    raw_json: &[u8],
    output_root: &Path,
) -> AppResult<Vec<PdfContentBlockDto>> {
    let output_root = canonical_existing_path(output_root, "opendataloader_output_root_invalid")?;
    let root: Value = serde_json::from_slice(raw_json).map_err(|err| {
        AppError::new(
            "opendataloader_json_invalid",
            "OpenDataLoader PDF 返回了无效 JSON。",
            "pdf_structure_validate",
            false,
        )
        .with_details(err.to_string())
    })?;
    let object = root
        .as_object()
        .ok_or_else(|| schema_error("JSON 根节点必须是对象"))?;
    let page_count = object
        .get("number of pages")
        .and_then(Value::as_i64)
        .ok_or_else(|| schema_error("缺少 number of pages"))?;
    if page_count <= 0 || page_count as usize != pages.len() {
        return Err(schema_error(&format!(
            "page count mismatch: parser={page_count}; ledger={}",
            pages.len()
        )));
    }
    let kids = object
        .get("kids")
        .and_then(Value::as_array)
        .ok_or_else(|| schema_error("缺少 kids 数组"))?;
    let page_map: HashMap<i64, &PdfStructurePage> =
        pages.iter().map(|page| (page.page_number, page)).collect();
    if page_map.len() != pages.len() {
        return Err(schema_error("page number duplicated in ledger"));
    }

    let mut blocks = Vec::new();
    let mut ordinal = 0_i64;
    let mut raw_json_bytes = 0_usize;
    let mut text_bytes = 0_usize;
    for (index, kid) in kids.iter().enumerate() {
        flatten_node(
            document_id,
            parse_id,
            kid,
            &format!("{index}"),
            None,
            true,
            &page_map,
            &output_root,
            0,
            &mut ordinal,
            &mut raw_json_bytes,
            &mut text_bytes,
            &mut blocks,
        )?;
    }
    link_captions(&mut blocks);
    ensure_every_page_has_block_or_fallback(
        document_id,
        parse_id,
        pages,
        &mut ordinal,
        &mut blocks,
    )?;
    Ok(blocks)
}

#[allow(clippy::too_many_arguments)]
fn flatten_node(
    document_id: &str,
    parse_id: &str,
    node: &Value,
    path: &str,
    parent_block_id: Option<String>,
    is_top_level: bool,
    pages: &HashMap<i64, &PdfStructurePage>,
    output_root: &Path,
    depth: usize,
    ordinal: &mut i64,
    raw_json_bytes: &mut usize,
    text_bytes: &mut usize,
    blocks: &mut Vec<PdfContentBlockDto>,
) -> AppResult<String> {
    if depth >= MAX_BLOCK_TREE_DEPTH {
        return Err(schema_error("content block tree exceeds maximum depth"));
    }
    if blocks.len() >= MAX_CONTENT_BLOCKS {
        return Err(schema_error("content block count exceeds safety limit"));
    }
    let object = node
        .as_object()
        .ok_or_else(|| schema_error(&format!("kids[{path}] must be an object")))?;
    let block_type = object
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .trim()
        .to_ascii_lowercase();
    if block_type.len() > 80 {
        return Err(schema_error("block type exceeds 80 characters"));
    }
    let page_number = object
        .get("page number")
        .and_then(Value::as_i64)
        .ok_or_else(|| schema_error(&format!("kids[{path}] missing page number")))?;
    let page = pages
        .get(&page_number)
        .ok_or_else(|| schema_error(&format!("unknown page number {page_number}")))?;
    let source_element_id = object.get("id").and_then(value_id);
    let identity = source_element_id.as_deref().unwrap_or(path);
    let block_id = stable_block_id(document_id, identity, path);
    let raw_bbox = object.get("bounding box").and_then(parse_raw_bbox);
    let bbox = raw_bbox.and_then(|bbox| normalize_pdf_bbox(bbox, page.geometry));
    let mut own_source_text = object
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if block_type == "table" {
        let table_text = extract_table_text(object, depth + 1)?;
        if !table_text.is_empty() {
            if !own_source_text.is_empty() {
                own_source_text.push('\n');
            }
            own_source_text.push_str(&table_text);
        }
    }
    *text_bytes = text_bytes
        .checked_add(own_source_text.len())
        .filter(|total| *total <= MAX_BLOCK_TEXT_TOTAL_BYTES)
        .ok_or_else(|| schema_error("content block text exceeds safety limit"))?;
    let mut source_text = own_source_text.clone();
    let is_visual = matches!(
        block_type.as_str(),
        "image" | "picture" | "figure" | "chart" | "diagram"
    );
    let source_image_path = match object.get("source").and_then(Value::as_str) {
        Some(source) => Some(validate_relative_output_path(output_root, source)?),
        None => None,
    };
    let is_decorative = is_visual
        && (bbox
            .map(|bbox| bbox.width < 0.02 || bbox.height < 0.02 || bbox.width * bbox.height < 0.002)
            .unwrap_or(false)
            || (bbox.is_none() && source_image_path.is_none()));
    let is_indexable = if is_visual {
        !is_decorative
    } else {
        is_top_level
    };
    let raw_node = serde_json::to_string(node).map_err(|err| {
        AppError::new(
            "opendataloader_json_serialize_failed",
            "无法保存 OpenDataLoader PDF 模块数据。",
            "pdf_structure_validate",
            false,
        )
        .with_details(err.to_string())
    })?;
    *raw_json_bytes = raw_json_bytes
        .checked_add(raw_node.len())
        .filter(|total| *total <= MAX_BLOCK_RAW_JSON_TOTAL_BYTES)
        .ok_or_else(|| schema_error("content block raw JSON exceeds safety limit"))?;
    let block_ordinal = *ordinal;
    *ordinal += 1;
    let block_index = blocks.len();
    blocks.push(PdfContentBlockDto {
        block_id: block_id.clone(),
        parse_id: parse_id.to_string(),
        document_id: document_id.to_string(),
        page_id: page.page_id.clone(),
        page_number,
        parent_block_id,
        source_element_id,
        ordinal: block_ordinal,
        block_type,
        source_text: source_text.clone(),
        enrichment_json: None,
        raw_json: raw_node,
        source_image_path,
        is_indexable,
        is_visual,
        is_decorative,
        bbox,
    });

    match object.get("kids") {
        Some(Value::Array(children)) => {
            for (child_index, child) in children.iter().enumerate() {
                let child_text = flatten_node(
                    document_id,
                    parse_id,
                    child,
                    &format!("{path}.{child_index}"),
                    Some(block_id.clone()),
                    false,
                    pages,
                    output_root,
                    depth + 1,
                    ordinal,
                    raw_json_bytes,
                    text_bytes,
                    blocks,
                )?;
                if !child_text.is_empty() {
                    if !source_text.is_empty() {
                        source_text.push('\n');
                    }
                    source_text.push_str(&child_text);
                }
            }
            if is_top_level {
                blocks[block_index].source_text = source_text.clone();
            }
        }
        Some(_) => return Err(schema_error(&format!("kids[{path}].kids must be an array"))),
        None => {}
    }
    Ok(source_text)
}

fn ensure_every_page_has_block_or_fallback(
    document_id: &str,
    parse_id: &str,
    pages: &[PdfStructurePage],
    ordinal: &mut i64,
    blocks: &mut Vec<PdfContentBlockDto>,
) -> AppResult<()> {
    let covered_pages: std::collections::HashSet<i64> =
        blocks.iter().map(|block| block.page_number).collect();
    for page in pages {
        if covered_pages.contains(&page.page_number) {
            continue;
        }
        if blocks.len() >= MAX_CONTENT_BLOCKS {
            return Err(schema_error("content block count exceeds safety limit"));
        }
        let path = format!("page-fallback-{}", page.page_number);
        blocks.push(PdfContentBlockDto {
            block_id: stable_block_id(document_id, &path, &path),
            parse_id: parse_id.to_string(),
            document_id: document_id.to_string(),
            page_id: page.page_id.clone(),
            page_number: page.page_number,
            parent_block_id: None,
            source_element_id: None,
            ordinal: *ordinal,
            block_type: "page_fallback".to_string(),
            source_text: String::new(),
            enrichment_json: None,
            raw_json: serde_json::json!({
                "type": "page_fallback",
                "page number": page.page_number,
                "reason": "no_structured_blocks"
            })
            .to_string(),
            source_image_path: None,
            is_indexable: false,
            is_visual: false,
            is_decorative: false,
            bbox: None,
        });
        *ordinal += 1;
    }
    Ok(())
}

fn extract_table_text(object: &serde_json::Map<String, Value>, depth: usize) -> AppResult<String> {
    if depth >= MAX_BLOCK_TREE_DEPTH {
        return Err(schema_error("table tree exceeds maximum depth"));
    }
    let Some(rows_value) = object.get("rows") else {
        return Ok(String::new());
    };
    let rows = rows_value
        .as_array()
        .ok_or_else(|| schema_error("table rows must be an array"))?;
    let mut row_texts = Vec::with_capacity(rows.len());
    for (row_index, row) in rows.iter().enumerate() {
        let row = row
            .as_object()
            .ok_or_else(|| schema_error(&format!("table rows[{row_index}] must be an object")))?;
        let cells = row
            .get("cells")
            .ok_or_else(|| schema_error(&format!("table rows[{row_index}] missing cells")))?
            .as_array()
            .ok_or_else(|| {
                schema_error(&format!("table rows[{row_index}].cells must be an array"))
            })?;
        let mut cell_texts = Vec::with_capacity(cells.len());
        for (cell_index, cell) in cells.iter().enumerate() {
            let mut parts = Vec::new();
            collect_nested_content(
                cell,
                depth + 1,
                &format!("rows[{row_index}].cells[{cell_index}]"),
                &mut parts,
            )?;
            cell_texts.push(parts.join("\n"));
        }
        row_texts.push(cell_texts.join("\t"));
    }
    Ok(row_texts.join("\n"))
}

fn collect_nested_content(
    value: &Value,
    depth: usize,
    path: &str,
    output: &mut Vec<String>,
) -> AppResult<()> {
    if depth >= MAX_BLOCK_TREE_DEPTH {
        return Err(schema_error("table tree exceeds maximum depth"));
    }
    let object = value
        .as_object()
        .ok_or_else(|| schema_error(&format!("table {path} must be an object")))?;
    if let Some(content) = object.get("content").and_then(Value::as_str) {
        let content = content.trim();
        if !content.is_empty() {
            output.push(content.to_string());
        }
    }
    match object.get("kids") {
        Some(Value::Array(kids)) => {
            for (index, kid) in kids.iter().enumerate() {
                collect_nested_content(kid, depth + 1, &format!("{path}.kids[{index}]"), output)?;
            }
        }
        Some(_) => return Err(schema_error(&format!("table {path}.kids must be an array"))),
        None => {}
    }
    Ok(())
}

fn link_captions(blocks: &mut [PdfContentBlockDto]) {
    let mut by_source_id: HashMap<(i64, String), Option<usize>> = HashMap::new();
    for (index, block) in blocks.iter().enumerate() {
        let Some(id) = block.source_element_id.clone() else {
            continue;
        };
        by_source_id
            .entry((block.page_number, id))
            .and_modify(|target| *target = None)
            .or_insert(Some(index));
    }
    let links: Vec<(usize, i64, String, String)> = blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| block.block_type == "caption")
        .filter_map(|(caption_index, block)| {
            let value: Value = serde_json::from_str(&block.raw_json).ok()?;
            let linked = value.get("linked content id").and_then(value_id)?;
            (!block.source_text.trim().is_empty()).then(|| {
                (
                    caption_index,
                    block.page_number,
                    linked,
                    block.source_text.trim().to_string(),
                )
            })
        })
        .collect();
    for (caption_index, page_number, linked_id, caption) in links {
        let Some(target_index) = by_source_id
            .get(&(page_number, linked_id))
            .and_then(|target| *target)
        else {
            continue;
        };
        if blocks[target_index].is_visual && blocks[target_index].is_indexable {
            if !blocks[target_index].source_text.is_empty() {
                blocks[target_index].source_text.push('\n');
            }
            blocks[target_index].source_text.push_str(&caption);
            blocks[caption_index].is_indexable = false;
        }
    }
}

fn value_id(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn parse_raw_bbox(value: &Value) -> Option<[f64; 4]> {
    let values = value.as_array()?;
    if values.len() != 4 {
        return None;
    }
    Some([
        values[0].as_f64()?,
        values[1].as_f64()?,
        values[2].as_f64()?,
        values[3].as_f64()?,
    ])
}

fn stable_block_id(document_id: &str, identity: &str, path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(document_id.as_bytes());
    hasher.update([0]);
    hasher.update(identity.as_bytes());
    hasher.update([0]);
    hasher.update(path.as_bytes());
    let digest = hasher.finalize();
    format!("pdfmod-{}", hex(&digest[..16]))
}

fn validate_relative_output_path(output_root: &Path, value: &str) -> AppResult<String> {
    let relative = Path::new(value);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(AppError::new(
            "opendataloader_output_path_invalid",
            "OpenDataLoader PDF 返回了不安全的输出路径。",
            "pdf_structure_validate",
            false,
        ));
    }
    let candidate = output_root.join(relative);
    if !candidate.exists() {
        return Ok(relative.to_string_lossy().replace('\\', "/"));
    }
    let candidate = canonical_existing_path(&candidate, "opendataloader_image_missing")?;
    ensure_descendant(
        output_root,
        &candidate,
        "opendataloader_output_path_invalid",
    )?;
    if !candidate.is_file() {
        return Err(AppError::new(
            "opendataloader_image_not_file",
            "OpenDataLoader PDF returned a source path that is not an image file.",
            "pdf_structure_validate",
            false,
        ));
    }
    let canonical_relative = candidate.strip_prefix(output_root).map_err(|_| {
        AppError::new(
            "opendataloader_output_path_invalid",
            "OpenDataLoader PDF returned an output path outside the extraction directory.",
            "pdf_structure_validate",
            false,
        )
    })?;
    Ok(canonical_relative.to_string_lossy().replace('\\', "/"))
}

fn validate_output_artifact(
    output_dir: &Path,
    file_name: &str,
    missing_code: &str,
    too_large_code: &str,
    max_bytes: u64,
) -> AppResult<PathBuf> {
    let path = canonical_existing_path(&output_dir.join(file_name), missing_code)?;
    ensure_descendant(output_dir, &path, "opendataloader_output_path_invalid")?;
    let metadata = fs::metadata(&path).map_err(|err| {
        AppError::io("pdf_structure_validate", missing_code, err)
            .with_details(path.display().to_string())
    })?;
    if !metadata.is_file() {
        return Err(AppError::new(
            missing_code,
            "OpenDataLoader PDF 输出文件缺失。",
            "pdf_structure_validate",
            false,
        )
        .with_details(path.display().to_string()));
    }
    if metadata.len() > max_bytes {
        return Err(AppError::new(
            too_large_code,
            "OpenDataLoader PDF 输出超过安全大小限制。",
            "pdf_structure_validate",
            false,
        )
        .with_details(format!("bytes={}; max_bytes={max_bytes}", metadata.len())));
    }
    Ok(path)
}

fn canonical_existing_path(path: &Path, code: &str) -> AppResult<PathBuf> {
    fs::canonicalize(path).map_err(|err| {
        AppError::io("pdf_structure_validate", code, err).with_details(path.display().to_string())
    })
}

fn ensure_descendant(root: &Path, candidate: &Path, code: &str) -> AppResult<()> {
    if candidate == root || candidate.starts_with(root) {
        return Ok(());
    }
    Err(AppError::new(
        code,
        "PDF 结构化处理拒绝了工作区之外的路径。",
        "pdf_structure_validate",
        false,
    ))
}

fn require_supported_java_version(stdout: &str, stderr: &str) -> AppResult<u32> {
    let output = format!("{stdout}\n{stderr}");
    let major = parse_java_major_version(&output).ok_or_else(|| {
        AppError::new(
            "java_version_unrecognized",
            "无法识别 Java 版本，请安装 Java 11 或更高版本。",
            "pdf_structure_health",
            true,
        )
        .with_details(output.trim())
    })?;
    if major < 11 {
        return Err(AppError::new(
            "java_version_unsupported",
            "Java 版本过低，请安装 Java 11 或更高版本。",
            "pdf_structure_health",
            true,
        )
        .with_details(format!("detected_java_major={major}")));
    }
    Ok(major)
}

fn parse_java_major_version(output: &str) -> Option<u32> {
    output.lines().find_map(|line| {
        let lowercase = line.to_ascii_lowercase();
        let marker = lowercase.find("version")?;
        line[marker + "version".len()..]
            .split_whitespace()
            .take(3)
            .find_map(parse_java_version_token)
    })
}

fn parse_java_version_token(token: &str) -> Option<u32> {
    let mut numbers = token
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u32>().ok());
    let first = numbers.next()?;
    if first == 1 {
        numbers.next().or(Some(first))
    } else {
        Some(first)
    }
}

fn validate_image_output_limits(image_dir: &Path, limits: ImageOutputLimits) -> AppResult<()> {
    let image_root = canonical_existing_path(image_dir, "opendataloader_image_dir_missing")?;
    let mut pending_dirs = vec![image_root.clone()];
    let mut file_count = 0_usize;
    let mut total_bytes = 0_u64;
    let mut total_pixels = 0_u64;

    while let Some(directory) = pending_dirs.pop() {
        let entries = fs::read_dir(&directory).map_err(|err| {
            AppError::io(
                "pdf_structure_validate",
                "opendataloader_image_dir_read_failed",
                err,
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|err| {
                AppError::io(
                    "pdf_structure_validate",
                    "opendataloader_image_entry_read_failed",
                    err,
                )
            })?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(|err| {
                AppError::io(
                    "pdf_structure_validate",
                    "opendataloader_image_metadata_failed",
                    err,
                )
            })?;
            if is_link_or_reparse_point(&metadata) {
                return Err(AppError::new(
                    "opendataloader_image_path_invalid",
                    "OpenDataLoader PDF 返回了不安全的图片路径。",
                    "pdf_structure_validate",
                    false,
                ));
            }

            let canonical_path =
                canonical_existing_path(&path, "opendataloader_image_path_invalid")?;
            ensure_descendant(
                &image_root,
                &canonical_path,
                "opendataloader_image_path_invalid",
            )?;
            if metadata.is_dir() {
                pending_dirs.push(canonical_path);
                continue;
            }
            if !metadata.is_file() {
                return Err(AppError::new(
                    "opendataloader_image_entry_invalid",
                    "OpenDataLoader PDF 返回了不支持的图片目录项。",
                    "pdf_structure_validate",
                    false,
                ));
            }

            file_count = file_count.checked_add(1).ok_or_else(|| {
                AppError::new(
                    "opendataloader_image_count_exceeded",
                    "OpenDataLoader PDF 提取的图片数量超过安全限制。",
                    "pdf_structure_validate",
                    false,
                )
            })?;
            if file_count > limits.max_count {
                return Err(AppError::new(
                    "opendataloader_image_count_exceeded",
                    "OpenDataLoader PDF 提取的图片数量超过安全限制。",
                    "pdf_structure_validate",
                    false,
                )
                .with_details(format!(
                    "count={file_count}; max_count={}",
                    limits.max_count
                )));
            }

            let file_bytes = fs::metadata(&canonical_path)
                .map_err(|err| {
                    AppError::io(
                        "pdf_structure_validate",
                        "opendataloader_image_metadata_failed",
                        err,
                    )
                })?
                .len();
            if file_bytes > limits.max_file_bytes {
                return Err(AppError::new(
                    "opendataloader_image_too_large",
                    "OpenDataLoader PDF 提取的单张图片超过安全大小限制。",
                    "pdf_structure_validate",
                    false,
                )
                .with_details(format!(
                    "bytes={file_bytes}; max_bytes={}",
                    limits.max_file_bytes
                )));
            }
            total_bytes = total_bytes.checked_add(file_bytes).ok_or_else(|| {
                AppError::new(
                    "opendataloader_images_too_large",
                    "OpenDataLoader PDF 提取的图片总量超过安全大小限制。",
                    "pdf_structure_validate",
                    false,
                )
            })?;
            if total_bytes > limits.max_total_bytes {
                return Err(AppError::new(
                    "opendataloader_images_too_large",
                    "OpenDataLoader PDF 提取的图片总量超过安全大小限制。",
                    "pdf_structure_validate",
                    false,
                )
                .with_details(format!(
                    "bytes={total_bytes}; max_bytes={}",
                    limits.max_total_bytes
                )));
            }

            let mut reader = image::ImageReader::open(&canonical_path)
                .and_then(|reader| reader.with_guessed_format())
                .map_err(|err| {
                    AppError::io(
                        "pdf_structure_validate",
                        "opendataloader_image_open_failed",
                        err,
                    )
                })?;
            let mut image_limits = image::Limits::default();
            image_limits.max_image_width = Some(limits.max_width);
            image_limits.max_image_height = Some(limits.max_height);
            image_limits.max_alloc = Some(256 * 1024 * 1024);
            reader.limits(image_limits);
            let (width, height) = reader.into_dimensions().map_err(|err| {
                AppError::new(
                    "opendataloader_image_invalid",
                    "OpenDataLoader PDF returned an invalid or unsupported image.",
                    "pdf_structure_validate",
                    false,
                )
                .with_details(err.to_string())
            })?;
            let pixels = u64::from(width)
                .checked_mul(u64::from(height))
                .ok_or_else(|| {
                    AppError::new(
                        "opendataloader_image_dimensions_exceeded",
                        "OpenDataLoader PDF returned an image with unsafe dimensions.",
                        "pdf_structure_validate",
                        false,
                    )
                })?;
            if pixels > limits.max_pixels {
                return Err(AppError::new(
                    "opendataloader_image_dimensions_exceeded",
                    "OpenDataLoader PDF returned an image with unsafe dimensions.",
                    "pdf_structure_validate",
                    false,
                )
                .with_details(format!(
                    "width={width}; height={height}; pixels={pixels}; max_pixels={}",
                    limits.max_pixels
                )));
            }
            total_pixels = total_pixels.checked_add(pixels).ok_or_else(|| {
                AppError::new(
                    "opendataloader_images_dimensions_exceeded",
                    "OpenDataLoader PDF returned too many decoded image pixels.",
                    "pdf_structure_validate",
                    false,
                )
            })?;
            if total_pixels > limits.max_total_pixels {
                return Err(AppError::new(
                    "opendataloader_images_dimensions_exceeded",
                    "OpenDataLoader PDF returned too many decoded image pixels.",
                    "pdf_structure_validate",
                    false,
                )
                .with_details(format!(
                    "pixels={total_pixels}; max_pixels={}",
                    limits.max_total_pixels
                )));
            }
        }
    }

    Ok(())
}

fn schema_error(details: &str) -> AppError {
    AppError::new(
        "opendataloader_schema_invalid",
        "OpenDataLoader PDF 输出不符合预期结构。",
        "pdf_structure_validate",
        false,
    )
    .with_details(details)
}

fn sha256_file(path: &Path) -> AppResult<String> {
    let mut file = fs::File::open(path).map_err(|err| {
        AppError::io(
            "pdf_structure_health",
            "opendataloader_jar_read_failed",
            err,
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|err| {
            AppError::io(
                "pdf_structure_health",
                "opendataloader_jar_read_failed",
                err,
            )
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[derive(Debug)]
struct BoundedProcessOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
    stdout_truncated: bool,
    stderr_truncated: bool,
}

impl BoundedProcessOutput {
    fn diagnostics(&self) -> String {
        format!(
            "exit={}; stdout={}{}; stderr={}{}",
            self.status,
            self.stdout,
            if self.stdout_truncated {
                " [truncated]"
            } else {
                ""
            },
            self.stderr,
            if self.stderr_truncated {
                " [truncated]"
            } else {
                ""
            }
        )
    }
}

#[cfg(windows)]
struct ProcessTreeGuard {
    job: HANDLE,
}

#[cfg(windows)]
impl ProcessTreeGuard {
    fn attach(child: &std::process::Child) -> std::io::Result<Self> {
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err(std::io::Error::last_os_error());
            }

            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) == 0
            {
                let error = std::io::Error::last_os_error();
                CloseHandle(job);
                return Err(error);
            }

            if AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE) == 0 {
                let error = std::io::Error::last_os_error();
                CloseHandle(job);
                return Err(error);
            }
            Ok(Self { job })
        }
    }

    fn terminate(&self, _child: &mut std::process::Child) -> std::io::Result<()> {
        if unsafe { TerminateJobObject(self.job, 1) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for ProcessTreeGuard {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.job);
        }
    }
}

#[cfg(not(windows))]
struct ProcessTreeGuard;

#[cfg(not(windows))]
impl ProcessTreeGuard {
    fn attach(_child: &std::process::Child) -> std::io::Result<Self> {
        Ok(Self)
    }

    fn terminate(&self, child: &mut std::process::Child) -> std::io::Result<()> {
        child.kill()
    }
}

fn run_bounded(
    program: &OsStr,
    args: &[OsString],
    timeout: Duration,
    failure_code: &str,
) -> AppResult<BoundedProcessOutput> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            let code = if err.kind() == std::io::ErrorKind::NotFound {
                "java_not_found"
            } else {
                failure_code
            };
            AppError::io("pdf_structure_process", code, err)
        })?;
    let mut process_tree = ProcessTreeGuard::attach(&child).map_err(|err| {
        let _ = child.kill();
        let _ = child.wait();
        AppError::io("pdf_structure_process", "process_tree_attach_failed", err)
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        AppError::new(
            failure_code,
            "无法读取 PDF 结构化进程输出。",
            "pdf_structure_process",
            true,
        )
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        AppError::new(
            failure_code,
            "无法读取 PDF 结构化进程错误输出。",
            "pdf_structure_process",
            true,
        )
    })?;
    let (stdout_sender, stdout_receiver) = mpsc::channel();
    let (stderr_sender, stderr_receiver) = mpsc::channel();
    let stdout_thread = thread::spawn(move || {
        let _ = stdout_sender.send(read_bounded(stdout, MAX_PROCESS_OUTPUT_BYTES));
    });
    let stderr_thread = thread::spawn(move || {
        let _ = stderr_sender.send(read_bounded(stderr, MAX_PROCESS_OUTPUT_BYTES));
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(25)),
            Ok(None) => {
                let termination = terminate_child_bounded(&mut child, &mut process_tree);
                drop(process_tree);
                let stdout_drain = receive_process_output(
                    stdout_receiver,
                    stdout_thread,
                    failure_code,
                    "读取 PDF 结构化标准输出超时。",
                );
                let stderr_drain = receive_process_output(
                    stderr_receiver,
                    stderr_thread,
                    failure_code,
                    "读取 PDF 结构化错误输出超时。",
                );
                return Err(AppError::new(
                    "opendataloader_timeout",
                    "OpenDataLoader PDF 处理超时，已终止进程。",
                    "pdf_structure_process",
                    true,
                )
                .with_details(format!(
                    "timeout_ms={}; termination={}; stdout_drain={}; stderr_drain={}",
                    timeout.as_millis(),
                    diagnostic_result(&termination),
                    diagnostic_result(&stdout_drain),
                    diagnostic_result(&stderr_drain),
                )));
            }
            Err(err) => {
                let termination = terminate_child_bounded(&mut child, &mut process_tree);
                drop(process_tree);
                let _ = receive_process_output(
                    stdout_receiver,
                    stdout_thread,
                    failure_code,
                    "读取 PDF 结构化标准输出超时。",
                );
                let _ = receive_process_output(
                    stderr_receiver,
                    stderr_thread,
                    failure_code,
                    "读取 PDF 结构化错误输出超时。",
                );
                let mut error = AppError::io("pdf_structure_process", failure_code, err);
                if let Err(termination_error) = termination {
                    error = error
                        .with_details(format!("process termination failed: {termination_error}"));
                }
                return Err(error);
            }
        }
    };
    drop(process_tree);
    let (stdout, stdout_truncated) = receive_process_output(
        stdout_receiver,
        stdout_thread,
        failure_code,
        "读取 PDF 结构化标准输出超时。",
    )?;
    let (stderr, stderr_truncated) = receive_process_output(
        stderr_receiver,
        stderr_thread,
        failure_code,
        "读取 PDF 结构化错误输出超时。",
    )?;
    let output = BoundedProcessOutput {
        status,
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
    };
    if !output.status.success() {
        return Err(AppError::new(
            failure_code,
            "OpenDataLoader PDF 处理失败。",
            "pdf_structure_process",
            true,
        )
        .with_details(output.diagnostics()));
    }
    Ok(output)
}

fn terminate_child_bounded(
    child: &mut std::process::Child,
    process_tree: &mut ProcessTreeGuard,
) -> AppResult<()> {
    let termination_result = process_tree.terminate(child);
    let started = Instant::now();
    while started.elapsed() < PROCESS_TERMINATION_GRACE {
        match child.try_wait() {
            Ok(Some(_)) => {
                return termination_result.map_err(|err| {
                    AppError::io(
                        "pdf_structure_process",
                        "process_tree_termination_failed",
                        err,
                    )
                })
            }
            Err(err) => {
                return Err(AppError::io(
                    "pdf_structure_process",
                    "process_termination_wait_failed",
                    err,
                ));
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
        }
    }
    Err(AppError::new(
        "process_termination_timeout",
        "PDF 结构化进程树未在终止宽限期内退出。",
        "pdf_structure_process",
        true,
    ))
}

fn receive_process_output(
    receiver: mpsc::Receiver<AppResult<(String, bool)>>,
    reader_thread: thread::JoinHandle<()>,
    failure_code: &str,
    timeout_message: &str,
) -> AppResult<(String, bool)> {
    match receiver.recv_timeout(PROCESS_OUTPUT_DRAIN_GRACE) {
        Ok(result) => {
            reader_thread.join().map_err(|_| {
                AppError::new(
                    failure_code,
                    "PDF 结构化输出读取线程异常终止。",
                    "pdf_structure_process",
                    true,
                )
            })?;
            result
        }
        Err(error) => {
            Err(
                AppError::new(failure_code, timeout_message, "pdf_structure_process", true)
                    .with_details(error.to_string()),
            )
        }
    }
}

fn diagnostic_result<T>(result: &AppResult<T>) -> &'static str {
    if result.is_ok() {
        "ok"
    } else {
        "failed"
    }
}

fn read_bounded(mut reader: impl Read, limit: usize) -> AppResult<(String, bool)> {
    let mut kept = Vec::with_capacity(limit.min(8192));
    let mut buffer = [0_u8; 8192];
    let mut truncated = false;
    loop {
        let count = reader.read(&mut buffer).map_err(|err| {
            AppError::io("pdf_structure_process", "process_output_read_failed", err)
        })?;
        if count == 0 {
            break;
        }
        let remaining = limit.saturating_sub(kept.len());
        let keep = remaining.min(count);
        kept.write_all(&buffer[..keep]).map_err(|err| {
            AppError::io("pdf_structure_process", "process_output_buffer_failed", err)
        })?;
        truncated |= keep < count;
    }
    Ok((String::from_utf8_lossy(&kept).into_owned(), truncated))
}

#[cfg(test)]
mod tests {
    use super::{
        parse_java_major_version, parse_opendataloader_json, read_bounded,
        require_supported_java_version, validate_image_output_limits, ImageOutputLimits,
    };
    use crate::domain::pdf_structure::{PdfPageGeometry, PdfStructurePage};
    use image::{ImageFormat, Rgb, RgbImage};
    use std::fs;
    use std::io::Cursor;

    fn page() -> PdfStructurePage {
        PdfStructurePage {
            page_id: "page-1".to_string(),
            page_number: 1,
            geometry: PdfPageGeometry {
                width_points: 100.0,
                height_points: 200.0,
                crop_left_points: 0.0,
                crop_bottom_points: 0.0,
                crop_right_points: 100.0,
                crop_top_points: 200.0,
                rotation_degrees: 0,
            },
        }
    }

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let image =
            image::DynamicImage::ImageRgb8(RgbImage::from_pixel(width, height, Rgb([20, 40, 60])));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .expect("encode png");
        bytes
    }

    fn image_limits(
        max_count: usize,
        max_file_bytes: u64,
        max_total_bytes: u64,
    ) -> ImageOutputLimits {
        ImageOutputLimits {
            max_count,
            max_file_bytes,
            max_total_bytes,
            max_width: 1_024,
            max_height: 1_024,
            max_pixels: 1_000_000,
            max_total_pixels: 2_000_000,
        }
    }

    #[test]
    fn parses_and_enforces_supported_java_major_versions() {
        assert_eq!(
            parse_java_major_version(
                "openjdk version \"17.0.12\" 2024-07-16\nOpenJDK Runtime Environment"
            ),
            Some(17)
        );
        assert_eq!(
            parse_java_major_version("java version \"1.8.0_402\"\nJava(TM) SE Runtime"),
            Some(8)
        );
        assert_eq!(
            require_supported_java_version("", "openjdk version \"21.0.2\"").expect("java 21"),
            21
        );
        assert_eq!(
            require_supported_java_version("openjdk version \"11.0.22\"", "")
                .expect("java 11 boundary"),
            11
        );
        assert_eq!(
            require_supported_java_version("", "openjdk version \"10.0.2\"")
                .expect_err("java 10 must be rejected")
                .code,
            "java_version_unsupported"
        );
        assert_eq!(
            require_supported_java_version("", "java version \"1.8.0_402\"")
                .expect_err("java 8 must be rejected")
                .code,
            "java_version_unsupported"
        );
        assert_eq!(
            require_supported_java_version("", "unexpected java launcher output")
                .expect_err("unrecognized version must be rejected")
                .code,
            "java_version_unrecognized"
        );
    }

    #[test]
    fn enforces_extracted_image_count_and_size_limits_recursively() {
        let root =
            std::env::temp_dir().join(format!("slicer-odl-image-limits-{}", uuid::Uuid::new_v4()));
        let one_pixel = png_bytes(1, 1);
        let png_size = one_pixel.len() as u64;
        let valid = root.join("valid");
        fs::create_dir_all(valid.join("nested")).expect("valid image dirs");
        fs::write(valid.join("one.png"), &one_pixel).expect("first image");
        fs::write(valid.join("nested/two.png"), &one_pixel).expect("nested image");
        validate_image_output_limits(&valid, image_limits(2, png_size, png_size * 2))
            .expect("valid nested images");

        let count = root.join("count");
        fs::create_dir_all(&count).expect("count dir");
        fs::write(count.join("one.png"), &one_pixel).expect("count one");
        fs::write(count.join("two.png"), &one_pixel).expect("count two");
        assert_eq!(
            validate_image_output_limits(&count, image_limits(1, png_size, png_size * 2))
                .expect_err("image count must be bounded")
                .code,
            "opendataloader_image_count_exceeded"
        );

        let single = root.join("single");
        fs::create_dir_all(&single).expect("single dir");
        fs::write(single.join("large.png"), &one_pixel).expect("large image");
        assert_eq!(
            validate_image_output_limits(&single, image_limits(1, png_size - 1, png_size))
                .expect_err("single image size must be bounded")
                .code,
            "opendataloader_image_too_large"
        );

        let total = root.join("total");
        fs::create_dir_all(&total).expect("total dir");
        fs::write(total.join("one.png"), &one_pixel).expect("total one");
        fs::write(total.join("two.png"), &one_pixel).expect("total two");
        assert_eq!(
            validate_image_output_limits(&total, image_limits(2, png_size, png_size * 2 - 1),)
                .expect_err("total image size must be bounded")
                .code,
            "opendataloader_images_too_large"
        );

        let dimensions = root.join("dimensions");
        fs::create_dir_all(&dimensions).expect("dimensions dir");
        let wide = png_bytes(10, 10);
        fs::write(dimensions.join("wide.png"), &wide).expect("dimension image");
        assert_eq!(
            validate_image_output_limits(
                &dimensions,
                ImageOutputLimits {
                    max_pixels: 99,
                    ..image_limits(1, wide.len() as u64, wide.len() as u64)
                },
            )
            .expect_err("decoded dimensions must be bounded")
            .code,
            "opendataloader_image_dimensions_exceeded"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parses_modules_links_captions_and_marks_visuals() {
        let root = std::env::temp_dir().join(format!("slicer-odl-json-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("images")).expect("images");
        fs::write(root.join("images/figure.png"), b"image").expect("image");
        let json = br#"{
          "number of pages": 1,
          "kids": [
            {"type":"paragraph","id":1,"page number":1,"bounding box":[10,150,90,180],"content":"Text"},
            {"type":"image","id":2,"page number":1,"bounding box":[10,50,90,140],"source":"images/figure.png"},
            {"type":"caption","id":3,"linked content id":2,"page number":1,"bounding box":[10,40,90,48],"content":"Figure caption"}
          ]
        }"#;
        let blocks =
            parse_opendataloader_json("doc", "parse", &[page()], json, &root).expect("parse");
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].source_text, "Text");
        assert!(blocks[1].is_visual);
        assert_eq!(blocks[1].source_text, "Figure caption");
        assert!(!blocks[2].is_indexable);
        assert_eq!(blocks[0].bbox.expect("bbox").y, 0.1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn indexes_nested_visuals_and_normalizes_source_paths() {
        let root =
            std::env::temp_dir().join(format!("slicer-odl-nested-visual-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("images")).expect("images");
        fs::write(root.join("images/figure.png"), b"image").expect("image");
        let json = br#"{
          "number of pages": 1,
          "kids": [{
            "type":"section","id":1,"page number":1,"content":"Overview",
            "kids":[{
              "type":"image","id":2,"page number":1,
              "bounding box":[10,50,90,140],"source":"./images/figure.png"
            }]
          }]
        }"#;

        let blocks =
            parse_opendataloader_json("doc", "parse", &[page()], json, &root).expect("parse");

        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].is_indexable);
        assert!(blocks[1].is_visual);
        assert!(blocks[1].is_indexable);
        assert_eq!(
            blocks[1].source_image_path.as_deref(),
            Some("images/figure.png")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_safe_missing_visual_source_without_dropping_structured_text() {
        let root =
            std::env::temp_dir().join(format!("slicer-odl-missing-image-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).expect("root");
        let json = br#"{
          "number of pages": 1,
          "kids": [
            {"type":"paragraph","id":1,"page number":1,"content":"Searchable text"},
            {"type":"image","id":2,"page number":1,
             "bounding box":[10,50,90,140],"source":"images/missing.png"}
          ]
        }"#;

        let blocks =
            parse_opendataloader_json("doc", "parse", &[page()], json, &root).expect("parse");

        assert_eq!(blocks[0].source_text, "Searchable text");
        assert_eq!(
            blocks[1].source_image_path.as_deref(),
            Some("images/missing.png")
        );
        assert!(blocks[1].is_visual);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_caption_indexable_when_linked_visual_is_decorative() {
        let root = std::env::temp_dir().join(format!(
            "slicer-odl-decorative-caption-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(root.join("images")).expect("images");
        fs::write(root.join("images/tiny.png"), b"image").expect("image");
        let json = br#"{
          "number of pages": 1,
          "kids": [
            {"type":"image","id":2,"page number":1,
             "bounding box":[10,10,10.1,10.1],"source":"images/tiny.png"},
            {"type":"caption","id":3,"linked content id":2,"page number":1,
             "content":"Caption remains searchable"}
          ]
        }"#;

        let blocks =
            parse_opendataloader_json("doc", "parse", &[page()], json, &root).expect("parse");

        assert!(blocks[0].is_decorative);
        assert!(blocks[1].is_indexable);
        assert_eq!(blocks[1].source_text, "Caption remains searchable");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn creates_non_analyzable_page_marker_when_parser_returns_no_blocks() {
        let root =
            std::env::temp_dir().join(format!("slicer-odl-page-fallback-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).expect("root");
        let blocks = parse_opendataloader_json(
            "doc",
            "parse",
            &[page()],
            br#"{"number of pages":1,"kids":[]}"#,
            &root,
        )
        .expect("fallback");

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, "page_fallback");
        assert!(!blocks[0].is_visual);
        assert!(!blocks[0].is_indexable);
        assert!(blocks[0].bbox.is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn aggregates_real_odl_rows_cells_kids_table_shape() {
        let json = br#"{
          "number of pages": 1,
          "kids": [{
            "type": "table",
            "id": 3,
            "page number": 1,
            "bounding box": [10, 10, 90, 190],
            "rows": [{
              "type": "table row",
              "row number": 1,
              "cells": [{
                "type": "table cell",
                "page number": 1,
                "kids": [{"type":"paragraph","id":1,"page number":1,"content":"Module"}]
              }, {
                "type": "table cell",
                "page number": 1,
                "kids": [{"type":"paragraph","id":2,"page number":1,"content":"Expected behavior"}]
              }]
            }, {
              "type": "table row",
              "row number": 2,
              "cells": [{
                "type": "table cell",
                "page number": 1,
                "kids": [{"type":"paragraph","id":3,"page number":1,"content":"Paragraph"}]
              }, {
                "type": "table cell",
                "page number": 1,
                "kids": [{"type":"paragraph","id":4,"page number":1,"content":"Indexed without a model request"}]
              }]
            }]
          }]
        }"#;

        let blocks =
            parse_opendataloader_json("doc", "parse", &[page()], json, &std::env::temp_dir())
                .expect("real table hierarchy");

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, "table");
        assert_eq!(
            blocks[0].source_text,
            "Module\tExpected behavior\nParagraph\tIndexed without a model request"
        );
        assert!(blocks[0].is_indexable);
    }

    #[test]
    fn rejects_directory_as_visual_source() {
        let root = std::env::temp_dir().join(format!(
            "slicer-odl-source-directory-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(root.join("images")).expect("images");
        let json = br#"{
          "number of pages": 1,
          "kids": [{
            "type":"image","id":1,"page number":1,
            "bounding box":[10,10,90,190],"source":"images"
          }]
        }"#;

        let error = parse_opendataloader_json("doc", "parse", &[page()], json, &root)
            .expect_err("directory source");
        assert_eq!(error.code, "opendataloader_image_not_file");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn links_reused_element_ids_to_captions_on_the_same_page() {
        let root =
            std::env::temp_dir().join(format!("slicer-odl-caption-pages-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("images")).expect("images");
        fs::write(root.join("images/figure.png"), b"image").expect("image");
        let mut second_page = page();
        second_page.page_id = "page-2".to_string();
        second_page.page_number = 2;
        let json = br#"{
          "number of pages": 2,
          "kids": [
            {"type":"image","id":2,"page number":1,"bounding box":[10,50,90,140],"source":"images/figure.png"},
            {"type":"caption","id":3,"linked content id":2,"page number":1,"bounding box":[10,40,90,48],"content":"Page one"},
            {"type":"image","id":2,"page number":2,"bounding box":[10,50,90,140],"source":"images/figure.png"},
            {"type":"caption","id":3,"linked content id":2,"page number":2,"bounding box":[10,40,90,48],"content":"Page two"}
          ]
        }"#;

        let blocks = parse_opendataloader_json("doc", "parse", &[page(), second_page], json, &root)
            .expect("parse");

        assert_eq!(blocks[0].source_text, "Page one");
        assert_eq!(blocks[2].source_text, "Page two");
        assert!(!blocks[1].is_indexable);
        assert!(!blocks[3].is_indexable);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_output_path_traversal() {
        let root = std::env::temp_dir().join(format!("slicer-odl-attack-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("root");
        let json = br#"{
          "number of pages": 1,
          "kids": [
            {"type":"image","id":1,"page number":1,"bounding box":[10,10,90,190],"source":"../secret.png"}
          ]
        }"#;
        let error = parse_opendataloader_json("doc", "parse", &[page()], json, &root)
            .expect_err("path traversal");
        assert_eq!(error.code, "opendataloader_output_path_invalid");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_page_count_mismatch_and_keeps_bad_bbox_as_null() {
        let root = std::env::temp_dir();
        let mismatch = br#"{"number of pages":2,"kids":[]}"#;
        assert_eq!(
            parse_opendataloader_json("doc", "parse", &[page()], mismatch, &root)
                .expect_err("mismatch")
                .code,
            "opendataloader_schema_invalid"
        );
        let bad_bbox = br#"{
          "number of pages":1,
          "kids":[{"type":"paragraph","id":1,"page number":1,"bounding box":[0,0,0,0],"content":"kept"}]
        }"#;
        let blocks = parse_opendataloader_json("doc", "parse", &[page()], bad_bbox, &root)
            .expect("bad bbox degrades");
        assert!(blocks[0].bbox.is_none());
    }

    #[test]
    fn rejects_non_array_nested_kids() {
        let json = br#"{
          "number of pages":1,
          "kids":[{
            "type":"paragraph",
            "id":1,
            "page number":1,
            "content":"parent",
            "kids":{"type":"paragraph","content":"silently lost before validation"}
          }]
        }"#;
        let error =
            parse_opendataloader_json("doc", "parse", &[page()], json, &std::env::temp_dir())
                .expect_err("nested kids must be an array");
        assert_eq!(error.code, "opendataloader_schema_invalid");
        assert!(error
            .details
            .as_deref()
            .unwrap_or_default()
            .contains("kids[0].kids must be an array"));
    }

    #[test]
    fn rejects_content_block_trees_beyond_depth_limit() {
        let mut node = serde_json::json!({
            "type": "paragraph",
            "id": "leaf",
            "page number": 1,
            "content": "leaf"
        });
        for depth in 0..super::MAX_BLOCK_TREE_DEPTH {
            node = serde_json::json!({
                "type": "paragraph",
                "id": format!("parent-{depth}"),
                "page number": 1,
                "kids": [node]
            });
        }
        let raw = serde_json::to_vec(&serde_json::json!({
            "number of pages": 1,
            "kids": [node]
        }))
        .expect("serialize deep fixture");

        let error =
            parse_opendataloader_json("doc", "parse", &[page()], &raw, &std::env::temp_dir())
                .expect_err("deep block trees must be rejected");
        assert_eq!(error.code, "opendataloader_schema_invalid");
        assert!(error
            .details
            .as_deref()
            .unwrap_or_default()
            .contains("maximum depth"));
    }

    #[cfg(windows)]
    #[test]
    fn bounded_process_attaches_to_windows_job_object() {
        let output = super::run_bounded(
            std::ffi::OsStr::new("cmd.exe"),
            &[
                std::ffi::OsString::from("/D"),
                std::ffi::OsString::from("/C"),
                std::ffi::OsString::from("echo job-object-ready"),
            ],
            std::time::Duration::from_secs(5),
            "test_process_failed",
        )
        .expect("bounded process");
        assert!(output.stdout.contains("job-object-ready"));
    }

    #[test]
    fn bounded_process_output_drains_input_and_reports_truncation() {
        let input = vec![b'x'; 128];
        let (output, truncated) = read_bounded(Cursor::new(input), 16).expect("bounded read");

        assert_eq!(output, "x".repeat(16));
        assert!(truncated);
    }
}
