use crate::domain::analysis::PageAnalysisV1;
use crate::domain::document::DocumentDto;
use crate::domain::page::PageRecordDto;
use crate::errors::{AppError, AppResult};
use crate::repositories::analysis_repository::AnalysisRepository;
use crate::repositories::document_repository::DocumentRepository;
use crate::services::workspace_service::WorkspaceService;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct MediaExportResult {
    pub markdown_path: String,
    pub export_dir: String,
    pub document_count: u32,
    pub media_count: u32,
}

#[derive(Debug, Clone, Serialize)]
struct MediaExportManifest {
    version: u32,
    exported_at: String,
    codes_order: Vec<String>,
    items: HashMap<String, MediaExportItem>,
}

#[derive(Debug, Clone, Serialize)]
struct MediaExportItem {
    manifest: MediaExportManifestEntry,
    backrefs: Vec<MediaBackref>,
}

#[derive(Debug, Clone, Serialize)]
struct MediaExportManifestEntry {
    code: String,
    sha256: String,
    mime: String,
    size: u64,
    rel_storage: String,
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    original_name: Option<String>,
    created_at: String,
}

#[derive(Debug, Clone, Serialize)]
struct MediaBackref {
    wiki_path: String,
    heading_path: String,
}

struct PageAnalysis {
    page: PageRecordDto,
    analysis: PageAnalysisV1,
}

pub struct MediaExporter;

impl MediaExporter {
    pub fn export(workspace: &WorkspaceService, destination: &Path) -> AppResult<MediaExportResult> {
        let layout = workspace.workspace_layout()?;
        let mut conn = workspace.get_db_connection()?;

        let documents = DocumentRepository::list_documents(&mut conn)?;
        if documents.is_empty() {
            return Err(AppError::new(
                "export_no_documents",
                "没有可导出的文档，请先导入并分析文档。",
                "export",
                false,
            ));
        }

        // Collect analyses per document
        let mut doc_analyses: Vec<(DocumentDto, Vec<PageAnalysis>)> = Vec::new();
        for doc in &documents {
            let pages = DocumentRepository::list_pages_by_document(&mut conn, &doc.document_id)?;
            let mut analyses = Vec::new();
            for page in &pages {
                if let Some(analysis) =
                    AnalysisRepository::find_succeeded_page_analysis(&mut conn, &page.page_id)?
                {
                    analyses.push(PageAnalysis {
                        page: page.clone(),
                        analysis,
                    });
                }
            }
            if !analyses.is_empty() {
                doc_analyses.push((doc.clone(), analyses));
            }
        }

        if doc_analyses.is_empty() {
            return Err(AppError::new(
                "export_no_analyses",
                "没有已分析的页面，请先完成页面分析。",
                "export",
                false,
            ));
        }

        // Determine markdown filename
        let markdown_filename = if doc_analyses.len() == 1 {
            let name = strip_extension(&doc_analyses[0].0.original_filename);
            format!("{name}.md")
        } else {
            "文案.md".to_string()
        };

        let wiki_path = markdown_filename.clone();
        let media_export_dir = destination.join("media-export");
        fs::create_dir_all(&media_export_dir).map_err(|e| {
            AppError::io("export", "export_dir_create_failed", e)
        })?;

        // Build markdown and collect media entries
        let mut markdown_content = String::new();
        let mut codes_order: Vec<String> = Vec::new();
        let mut items: HashMap<String, MediaExportItem> = HashMap::new();
        let mut copied_hashes: HashSet<String> = HashSet::new();

        for (doc, analyses) in &doc_analyses {
            let doc_name = strip_extension(&doc.original_filename);
            markdown_content.push_str(&format!("# {doc_name}\n\n"));

            for pa in analyses {
                let default_heading = format!("第 {} 页", pa.page.page_number);
                let heading = pa
                    .analysis
                    .analysis
                    .title
                    .as_deref()
                    .unwrap_or(&default_heading);

                markdown_content.push_str(&format!("## {heading}\n\n"));

                if let Some(ref text) = pa.analysis.analysis.visible_text {
                    if !text.is_empty() {
                        markdown_content.push_str(&format!("{text}\n\n"));
                    }
                }

                markdown_content
                    .push_str(&format!("![[MEDIA:{}]]\n\n", pa.page.image_hash));

                // Build media manifest entry
                let hash = &pa.page.image_hash;
                if !items.contains_key(hash) {
                    let source_path = layout.root().join(&pa.analysis.image_path);
                    let ext = extension_from_path(&pa.analysis.image_path);
                    let rel_storage = rel_storage_path(hash, ext);
                    let dest_path = media_export_dir.join(&rel_storage);

                    // Read and hash the file
                    let file_bytes = fs::read(&source_path).map_err(|e| {
                        AppError::io("export", "export_image_read_failed", e)
                    })?;
                    let sha256 = compute_sha256(&file_bytes);
                    let file_size = file_bytes.len() as u64;
                    let mime = mime_from_extension(ext);

                    // Copy file
                    if !copied_hashes.contains(hash) {
                        if let Some(parent) = dest_path.parent() {
                            fs::create_dir_all(parent).map_err(|e| {
                                AppError::io("export", "export_image_copy_failed", e)
                            })?;
                        }
                        fs::copy(&source_path, &dest_path).map_err(|e| {
                            AppError::io("export", "export_image_copy_failed", e)
                        })?;
                        copied_hashes.insert(hash.clone());
                    }

                    let created_at_str = chrono::Utc::now().to_rfc3339();

                    items.insert(
                        hash.clone(),
                        MediaExportItem {
                            manifest: MediaExportManifestEntry {
                                code: hash.clone(),
                                sha256,
                                mime: mime.to_string(),
                                size: file_size,
                                rel_storage,
                                title: pa.analysis.analysis.title.clone(),
                                original_name: pa.analysis.source.original_filename.clone(),
                                created_at: created_at_str,
                            },
                            backrefs: Vec::new(),
                        },
                    );
                    codes_order.push(hash.clone());
                }

                // Add backref
                if let Some(item) = items.get_mut(hash) {
                    item.backrefs.push(MediaBackref {
                        wiki_path: wiki_path.clone(),
                        heading_path: heading.to_string(),
                    });
                }
            }
        }

        // Write markdown file
        let markdown_path = destination.join(&markdown_filename);
        atomic_write_str(&markdown_path, &markdown_content)?;

        // Write manifest JSON
        let manifest = MediaExportManifest {
            version: 1,
            exported_at: chrono::Utc::now().to_rfc3339(),
            codes_order,
            items,
        };
        let manifest_json = serde_json::to_string_pretty(&manifest).map_err(|e| {
            AppError::new(
                "export_json_serialize_failed",
                "导出清单序列化失败。",
                "export",
                false,
            )
            .with_details(e.to_string())
        })?;
        let manifest_path = media_export_dir.join("pathy_media_export.json");
        atomic_write_str(&manifest_path, &manifest_json)?;

        Ok(MediaExportResult {
            markdown_path: markdown_path.to_string_lossy().into_owned(),
            export_dir: media_export_dir.to_string_lossy().into_owned(),
            document_count: doc_analyses.len() as u32,
            media_count: copied_hashes.len() as u32,
        })
    }
}

fn strip_extension(filename: &str) -> &str {
    filename.rfind('.').map_or(filename, |i| &filename[..i])
}

fn extension_from_path(path: &str) -> &str {
    path.rfind('.').map_or("png", |i| &path[i + 1..])
}

fn rel_storage_path(hash: &str, ext: &str) -> String {
    if hash.len() >= 4 {
        format!("objects/{}/{}/{}.{}", &hash[..2], &hash[2..4], hash, ext)
    } else {
        format!("objects/xx/xx/{}.{}", hash, ext)
    }
}

fn mime_from_extension(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        _ => "image/png",
    }
}

fn compute_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let result = hasher.finalize();
    format!("{:x}", result)
}

fn atomic_write_str(path: &Path, content: &str) -> AppResult<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, content).map_err(|e| AppError::io("export", "export_write_failed", e))?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        AppError::io("export", "export_write_failed", e)
    })?;
    Ok(())
}
