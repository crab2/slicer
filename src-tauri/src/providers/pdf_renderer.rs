use crate::errors::{AppError, AppResult};
use image::ImageFormat;
use pdfium_render::prelude::*;
use std::io::Cursor;
use std::path::Path;

pub struct RenderedPage {
    pub page_number: i64,
    pub png_bytes: Vec<u8>,
    pub image_hash: String,
}

pub trait PdfRenderer: Send + Sync {
    fn render_pdf(&self, pdf_path: &Path, dpi: f32) -> AppResult<Vec<RenderedPage>>;
    fn page_count(&self, pdf_path: &Path) -> AppResult<i64>;
}

pub struct PdfiumRenderer;

impl PdfRenderer for PdfiumRenderer {
    fn render_pdf(&self, pdf_path: &Path, _dpi: f32) -> AppResult<Vec<RenderedPage>> {
        let pdfium = load_pdfium()?;
        let document = pdfium.load_pdf_from_file(pdf_path, None).map_err(|e| {
            AppError::new(
                "pdf_load_failed",
                "无法加载 PDF 文件，文件可能已损坏或加密。",
                "pdf_render",
                true,
            )
            .with_details(format!("{e}"))
        })?;

        let mut pages = Vec::new();
        let page_count = document.pages().len();

        for i in 0..page_count {
            let page = document.pages().get(i).map_err(|e| {
                AppError::new(
                    "pdf_page_render_failed",
                    "PDF 页面渲染失败。",
                    "pdf_render",
                    true,
                )
                .with_details(format!("{e}"))
            })?;

            let bitmap = page
                .render_with_config(
                    &PdfRenderConfig::new()
                        .set_target_width(2000)
                        .set_maximum_height(2000),
                )
                .map_err(|e| {
                    AppError::new(
                        "pdf_page_render_failed",
                        "PDF 页面渲染失败。",
                        "pdf_render",
                        true,
                    )
                    .with_details(format!("{e}"))
                })?;

            let rgba_bytes = bitmap.as_rgba_bytes();
            let width = bitmap.width() as u32;
            let height = bitmap.height() as u32;

            let img = image::RgbaImage::from_raw(width, height, rgba_bytes.to_vec()).ok_or_else(
                || {
                    AppError::new(
                        "pdf_image_conversion_failed",
                        "PDF 页面图像转换失败。",
                        "pdf_render",
                        false,
                    )
                },
            )?;

            let mut png_buf = Vec::new();
            let mut cursor = Cursor::new(&mut png_buf);
            img.write_to(&mut cursor, ImageFormat::Png).map_err(|e| {
                AppError::new(
                    "pdf_png_encode_failed",
                    "PNG 编码失败。",
                    "pdf_render",
                    false,
                )
                .with_details(e.to_string())
            })?;

            let image_hash = compute_image_hash(&png_buf);

            pages.push(RenderedPage {
                page_number: i as i64 + 1,
                png_bytes: png_buf,
                image_hash,
            });
        }

        Ok(pages)
    }

    fn page_count(&self, pdf_path: &Path) -> AppResult<i64> {
        let pdfium = load_pdfium()?;
        let document = pdfium.load_pdf_from_file(pdf_path, None).map_err(|e| {
            AppError::new(
                "pdf_load_failed",
                "无法加载 PDF 文件，文件可能已损坏或加密。",
                "pdf_render",
                true,
            )
            .with_details(format!("{e}"))
        })?;

        Ok(document.pages().len() as i64)
    }
}

fn load_pdfium() -> AppResult<Pdfium> {
    pdfium_auto::bind_pdfium_silent().map_err(|e| {
        AppError::new(
            "pdfium_unavailable",
            "PDF 渲染库不可用，无法自动获取或加载 pdfium。",
            "pdf_render",
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
    let data = std::fs::read(path).map_err(|e| {
        AppError::new("file_read_failed", "无法读取文件。", "import", true)
            .with_details(e.to_string())
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(hex::encode(hasher.finalize()))
}

/// Hex encoding helper.
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
    }
}
