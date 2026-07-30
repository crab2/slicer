use crate::domain::pdf_structure::PdfPageGeometry;
use crate::errors::{AppError, AppResult};
use image::ImageFormat;
use pdfium_render::prelude::*;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;

pub const MAX_PDF_PAGE_COUNT: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PdfPageMetadata {
    pub page_number: i64,
    pub geometry: PdfPageGeometry,
}

pub trait PdfRenderer: Send + Sync {
    fn inspect_pdf(&self, pdf_path: &Path) -> AppResult<Vec<PdfPageMetadata>>;
}

pub struct PdfiumRenderer;

pub const SEARCH_PREVIEW_WIDTH: i32 = 1_400;
pub const SEARCH_PREVIEW_MAX_HEIGHT: i32 = 2_000;

impl PdfRenderer for PdfiumRenderer {
    fn inspect_pdf(&self, pdf_path: &Path) -> AppResult<Vec<PdfPageMetadata>> {
        let pdfium = load_pdfium()?;
        let document = pdfium.load_pdf_from_file(pdf_path, None).map_err(|e| {
            AppError::new(
                "pdf_load_failed",
                "无法加载 PDF 文件，文件可能已损坏或加密。",
                "pdf_metadata",
                true,
            )
            .with_details(format!("{e}"))
        })?;

        let page_count = document.pages().len();
        validate_pdf_page_count(usize::from(page_count))?;

        let mut pages = Vec::with_capacity(usize::from(page_count));

        for i in 0..page_count {
            let page = document.pages().get(i).map_err(|e| {
                AppError::new(
                    "pdf_page_metadata_failed",
                    "PDF 页面元数据读取失败。",
                    "pdf_metadata",
                    true,
                )
                .with_details(format!("{e}"))
            })?;

            let page_width_points = f64::from(page.width().value);
            let page_height_points = f64::from(page.height().value);
            let crop = page
                .boundaries()
                .crop()
                .or_else(|_| page.boundaries().media())
                .map(|boundary| boundary.bounds)
                .unwrap_or_else(|_| page.page_size());
            let rotation_degrees = page.rotation().map_err(|e| {
                AppError::new(
                    "pdf_page_geometry_failed",
                    "无法读取 PDF 页面旋转信息。",
                    "pdf_metadata",
                    false,
                )
                .with_details(format!("{e}"))
            })? as i64;
            let rotation_degrees = match rotation_degrees {
                0 => 0,
                1 => 90,
                2 => 180,
                3 => 270,
                _ => 0,
            };
            let geometry = PdfPageGeometry {
                width_points: page_width_points,
                height_points: page_height_points,
                crop_left_points: f64::from(crop.left().value),
                crop_bottom_points: f64::from(crop.bottom().value),
                crop_right_points: f64::from(crop.right().value),
                crop_top_points: f64::from(crop.top().value),
                rotation_degrees,
            };
            if !geometry.is_valid() {
                return Err(AppError::new(
                    "pdf_page_geometry_invalid",
                    "PDF 页面尺寸或 CropBox 无效。",
                    "pdf_metadata",
                    false,
                ));
            }
            pages.push(PdfPageMetadata {
                page_number: i as i64 + 1,
                geometry,
            });
        }

        Ok(pages)
    }
}

pub fn render_pdf_page_to_png(pdf_bytes: &[u8], page_number: i64) -> AppResult<Vec<u8>> {
    let pdfium = load_pdfium()?;
    let document = pdfium
        .load_pdf_from_byte_slice(pdf_bytes, None)
        .map_err(|error| {
            AppError::new(
                "pdf_preview_load_failed",
                "无法加载搜索预览 PDF。",
                "search",
                true,
            )
            .with_details(error.to_string())
        })?;
    let page_count = usize::from(document.pages().len());
    validate_pdf_page_count(page_count)?;
    let page_index = validate_page_number(page_number, page_count)?;
    let page = document.pages().get(page_index).map_err(|error| {
        AppError::new(
            "pdf_preview_page_read_failed",
            "无法读取搜索结果所在页面。",
            "search",
            true,
        )
        .with_details(error.to_string())
    })?;
    let bitmap = page
        .render_with_config(
            &PdfRenderConfig::new()
                .set_target_width(SEARCH_PREVIEW_WIDTH)
                .set_maximum_height(SEARCH_PREVIEW_MAX_HEIGHT),
        )
        .map_err(|error| {
            AppError::new(
                "pdf_preview_render_failed",
                "搜索结果页面渲染失败。",
                "search",
                true,
            )
            .with_details(error.to_string())
        })?;
    let mut output = Cursor::new(Vec::new());
    bitmap
        .as_image()
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|error| {
            AppError::new(
                "pdf_preview_encode_failed",
                "搜索结果页面编码失败。",
                "search",
                true,
            )
            .with_details(error.to_string())
        })?;
    Ok(output.into_inner())
}

fn validate_page_number(page_number: i64, page_count: usize) -> AppResult<PdfPageIndex> {
    let page_number = usize::try_from(page_number).map_err(|_| {
        AppError::new(
            "pdf_preview_page_out_of_range",
            "搜索结果页码超出 PDF 范围。",
            "search",
            false,
        )
        .with_details(format!(
            "page_number={page_number}; page_count={page_count}"
        ))
    })?;
    if page_number == 0 || page_number > page_count {
        return Err(AppError::new(
            "pdf_preview_page_out_of_range",
            "搜索结果页码超出 PDF 范围。",
            "search",
            false,
        )
        .with_details(format!(
            "page_number={page_number}; page_count={page_count}"
        )));
    }
    Ok((page_number - 1) as PdfPageIndex)
}

fn validate_pdf_page_count(page_count: usize) -> AppResult<()> {
    if page_count == 0 {
        return Err(AppError::new(
            "pdf_empty_document",
            "PDF 文件没有页面。",
            "pdf_metadata",
            false,
        ));
    }
    if page_count > MAX_PDF_PAGE_COUNT {
        return Err(AppError::new(
            "pdf_page_count_limit_exceeded",
            format!("PDF 页数超过安全上限 {MAX_PDF_PAGE_COUNT}。"),
            "pdf_metadata",
            false,
        )
        .with_details(format!("page_count={page_count}")));
    }
    Ok(())
}

fn load_pdfium() -> AppResult<Pdfium> {
    pdfium_auto::bind_pdfium_silent().map_err(|e| {
        AppError::new(
            "pdfium_unavailable",
            "PDF 元数据读取库不可用，无法自动获取或加载 pdfium。",
            "pdf_metadata",
            true,
        )
        .with_details(format!("{e}"))
    })
}

pub fn compute_image_hash(png_bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(png_bytes);
    let result = hasher.finalize();
    hex::encode(result)
}

/// Sanitize a filename for safe filesystem use.
pub fn sanitize_filename(name: &str) -> String {
    let invalid = ['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0'];
    name.chars()
        .map(|c| if invalid.contains(&c) { '_' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Compute SHA-256 hash of a file.
pub fn compute_file_hash(path: &Path) -> AppResult<String> {
    use sha2::{Digest, Sha256};
    let mut file = File::open(path).map_err(|e| {
        AppError::new("file_read_failed", "无法读取文件。", "import", true)
            .with_details(e.to_string())
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|e| {
            AppError::new("file_read_failed", "无法读取文件。", "import", true)
                .with_details(e.to_string())
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Hex encoding helper.
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        render_pdf_page_to_png, validate_page_number, validate_pdf_page_count, MAX_PDF_PAGE_COUNT,
    };

    #[test]
    fn rejects_empty_and_oversized_pdf_page_counts() {
        assert_eq!(
            validate_pdf_page_count(0).expect_err("empty PDF").code,
            "pdf_empty_document"
        );
        validate_pdf_page_count(MAX_PDF_PAGE_COUNT).expect("maximum accepted");
        assert_eq!(
            validate_pdf_page_count(MAX_PDF_PAGE_COUNT + 1)
                .expect_err("oversized PDF")
                .code,
            "pdf_page_count_limit_exceeded"
        );
    }

    #[test]
    fn validates_one_based_search_preview_page_numbers() {
        assert_eq!(validate_page_number(1, 4).expect("first page"), 0);
        assert_eq!(validate_page_number(4, 4).expect("last page"), 3);
        for invalid in [0, -1, 5] {
            assert_eq!(
                validate_page_number(invalid, 4)
                    .expect_err("invalid page")
                    .code,
                "pdf_preview_page_out_of_range"
            );
        }
    }

    #[test]
    fn renders_only_the_requested_pdf_page_to_memory_png() {
        let pdf = include_bytes!("../../../tmp/pdfs/structured-retrieval-fixture.pdf");
        let png = render_pdf_page_to_png(pdf, 2).expect("render second page");
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
    }
}
