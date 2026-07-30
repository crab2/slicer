use crate::errors::{AppError, AppResult};
use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub(crate) fn is_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceLayout {
    root: PathBuf,
}

impl WorkspaceLayout {
    pub fn from_root(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn originals_dir(&self) -> PathBuf {
        self.root.join("originals")
    }

    pub fn pages_dir(&self) -> PathBuf {
        self.root.join("pages")
    }

    pub fn analysis_dir(&self) -> PathBuf {
        self.root.join("analysis")
    }

    pub fn canonical_pdfs_dir(&self) -> PathBuf {
        self.root.join("pdfs")
    }

    pub fn document_pdf_dir(&self, document_id: &str) -> PathBuf {
        self.canonical_pdfs_dir().join(document_id)
    }

    pub fn canonical_pdf_path(&self, document_id: &str) -> PathBuf {
        self.document_pdf_dir(document_id).join("canonical.pdf")
    }

    pub fn pdf_structure_dir(&self) -> PathBuf {
        self.root.join("structure")
    }

    pub fn document_pdf_structure_dir(&self, document_id: &str) -> PathBuf {
        self.pdf_structure_dir().join(document_id)
    }

    pub fn pdf_structure_staging_dir(&self, parse_id: &str) -> PathBuf {
        self.tmp_dir().join(format!("pdf-structure-{parse_id}"))
    }

    pub fn metadata_dir(&self) -> PathBuf {
        self.root.join("metadata")
    }

    pub fn pages_jsonl_path(&self) -> PathBuf {
        self.metadata_dir().join("pages.jsonl")
    }

    pub fn indexes_dir(&self) -> PathBuf {
        self.root.join("indexes")
    }

    pub fn bm25_index_dir(&self) -> PathBuf {
        self.indexes_dir().join("bm25")
    }

    pub fn bm25_active_pointer_path(&self) -> PathBuf {
        self.bm25_index_dir().join("active.json")
    }

    pub fn bm25_build_dir(&self, version_id: &str) -> PathBuf {
        self.bm25_index_dir().join(format!("build-{version_id}"))
    }

    pub fn jobs_dir(&self) -> PathBuf {
        self.root.join("jobs")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn tmp_dir(&self) -> PathBuf {
        self.root.join("tmp")
    }

    pub fn app_db_path(&self) -> PathBuf {
        self.root.join("app.db")
    }

    pub fn validate_storage_id(&self, value: &str, kind: &str) -> AppResult<()> {
        if Uuid::parse_str(value).is_err()
            || Path::new(value).components().count() != 1
            || Path::new(value).file_name().and_then(|part| part.to_str()) != Some(value)
        {
            return Err(AppError::new(
                "workspace_storage_id_invalid",
                format!("{kind} 标识无效，已拒绝访问工作区制品。"),
                "workspace_boundary",
                false,
            ));
        }
        Ok(())
    }

    pub fn managed_parent(&self, parent: &Path) -> AppResult<PathBuf> {
        let root = fs::canonicalize(&self.root).map_err(|err| {
            AppError::io("workspace_boundary", "workspace_canonicalize_failed", err)
        })?;
        let metadata = fs::symlink_metadata(parent).map_err(|err| {
            AppError::io(
                "workspace_boundary",
                "workspace_managed_dir_metadata_failed",
                err,
            )
        })?;
        if parent != self.root && is_link_or_reparse_point(&metadata) {
            return Err(AppError::new(
                "workspace_managed_dir_link_rejected",
                "工作区制品目录不能是符号链接或 junction。",
                "workspace_boundary",
                false,
            )
            .with_details(parent.to_string_lossy().to_string()));
        }
        let parent = fs::canonicalize(parent).map_err(|err| {
            AppError::io(
                "workspace_boundary",
                "workspace_managed_dir_canonicalize_failed",
                err,
            )
        })?;
        if parent == root || parent.starts_with(&root) {
            return Ok(parent);
        }
        Err(AppError::new(
            "workspace_path_outside_root",
            "工作区制品目录越界，已拒绝文件操作。",
            "workspace_boundary",
            false,
        )
        .with_details(parent.to_string_lossy().to_string()))
    }

    pub fn ensure_managed_document_dir(
        &self,
        parent: &Path,
        document_id: &str,
    ) -> AppResult<PathBuf> {
        self.validate_storage_id(document_id, "document")?;
        let parent = self.managed_parent(parent)?;
        let candidate = parent.join(document_id);
        fs::create_dir(&candidate)
            .or_else(|err| {
                if err.kind() == std::io::ErrorKind::AlreadyExists {
                    Ok(())
                } else {
                    Err(err)
                }
            })
            .map_err(|err| {
                AppError::io(
                    "workspace_boundary",
                    "workspace_document_dir_create_failed",
                    err,
                )
            })?;
        self.resolve_existing_managed_document_dir(&parent, document_id)?
            .ok_or_else(|| {
                AppError::new(
                    "workspace_document_dir_missing",
                    "工作区文档制品目录创建后不可访问。",
                    "workspace_boundary",
                    true,
                )
            })
    }

    pub fn resolve_existing_managed_document_dir(
        &self,
        parent: &Path,
        document_id: &str,
    ) -> AppResult<Option<PathBuf>> {
        self.validate_storage_id(document_id, "document")?;
        let parent = self.managed_parent(parent)?;
        let candidate = parent.join(document_id);
        if !candidate.exists() {
            return Ok(None);
        }
        let metadata = fs::symlink_metadata(&candidate).map_err(|err| {
            AppError::io(
                "workspace_boundary",
                "workspace_document_dir_metadata_failed",
                err,
            )
        })?;
        if is_link_or_reparse_point(&metadata) {
            return Err(AppError::new(
                "workspace_document_dir_link_rejected",
                "工作区文档制品目录不能是符号链接或 junction。",
                "workspace_boundary",
                false,
            )
            .with_details(candidate.to_string_lossy().to_string()));
        }
        let resolved = fs::canonicalize(&candidate).map_err(|err| {
            AppError::io(
                "workspace_boundary",
                "workspace_document_dir_canonicalize_failed",
                err,
            )
        })?;
        if !resolved.starts_with(&parent) || !resolved.is_dir() {
            return Err(AppError::new(
                "workspace_document_dir_outside_parent",
                "工作区文档制品目录越界，已拒绝文件操作。",
                "workspace_boundary",
                false,
            )
            .with_details(resolved.to_string_lossy().to_string()));
        }
        Ok(Some(resolved))
    }

    pub fn ensure_base_layout(&self) -> AppResult<()> {
        let required_dirs = self.required_dirs();
        for dir in &required_dirs {
            self.ensure_managed_dir(dir)?;
        }

        self.ensure_managed_file(&self.app_db_path(), "workspace_app_db_failed")?;
        self.ensure_managed_file(&self.pages_jsonl_path(), "workspace_pages_jsonl_failed")?;

        Ok(())
    }

    fn ensure_managed_dir(&self, path: &Path) -> AppResult<()> {
        let root = fs::canonicalize(&self.root).map_err(|err| {
            AppError::io("workspace_boundary", "workspace_canonicalize_failed", err)
        })?;
        let relative = path.strip_prefix(&self.root).map_err(|_| {
            AppError::new(
                "workspace_path_outside_root",
                "工作区目录不在已选择的工作区内。",
                "workspace_boundary",
                false,
            )
        })?;
        let mut current = root.clone();
        for component in relative.components() {
            let std::path::Component::Normal(component) = component else {
                return Err(AppError::new(
                    "workspace_path_invalid",
                    "工作区目录包含无效路径组件。",
                    "workspace_boundary",
                    false,
                ));
            };
            let candidate = current.join(component);
            match fs::symlink_metadata(&candidate) {
                Ok(_) => {}
                Err(err) if err.kind() == ErrorKind::NotFound => {
                    fs::create_dir(&candidate).map_err(|err| {
                        AppError::io("initialize", "workspace_create_dir_failed", err)
                    })?;
                }
                Err(err) => {
                    return Err(AppError::io(
                        "workspace_boundary",
                        "workspace_managed_dir_metadata_failed",
                        err,
                    ));
                }
            }
            let metadata = fs::symlink_metadata(&candidate).map_err(|err| {
                AppError::io(
                    "workspace_boundary",
                    "workspace_managed_dir_metadata_failed",
                    err,
                )
            })?;
            if is_link_or_reparse_point(&metadata) || !metadata.is_dir() {
                return Err(AppError::new(
                    "workspace_managed_dir_link_rejected",
                    "工作区受管目录不能是符号链接、junction 或普通文件。",
                    "workspace_boundary",
                    false,
                )
                .with_details(candidate.to_string_lossy().to_string()));
            }
            current = fs::canonicalize(&candidate).map_err(|err| {
                AppError::io(
                    "workspace_boundary",
                    "workspace_managed_dir_canonicalize_failed",
                    err,
                )
            })?;
            if !current.starts_with(&root) {
                return Err(AppError::new(
                    "workspace_path_outside_root",
                    "工作区受管目录越界，已拒绝文件操作。",
                    "workspace_boundary",
                    false,
                )
                .with_details(current.to_string_lossy().to_string()));
            }
        }
        Ok(())
    }

    fn ensure_managed_file(&self, path: &Path, error_code: &str) -> AppResult<()> {
        let parent = path.parent().ok_or_else(|| {
            AppError::new(
                "workspace_managed_file_parent_missing",
                "工作区受管文件缺少父目录。",
                "workspace_boundary",
                false,
            )
        })?;
        let parent = self.managed_parent(parent)?;
        let name = path.file_name().ok_or_else(|| {
            AppError::new(
                "workspace_managed_file_name_missing",
                "工作区受管文件名无效。",
                "workspace_boundary",
                false,
            )
        })?;
        let candidate = parent.join(name);
        if let Ok(metadata) = fs::symlink_metadata(&candidate) {
            if is_link_or_reparse_point(&metadata) || !metadata.is_file() {
                return Err(AppError::new(
                    "workspace_managed_file_link_rejected",
                    "工作区受管文件不能是符号链接、reparse point 或目录。",
                    "workspace_boundary",
                    false,
                )
                .with_details(candidate.to_string_lossy().to_string()));
            }
        }

        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
            options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        }
        let file = options
            .open(&candidate)
            .map_err(|err| AppError::io("initialize", error_code, err))?;
        let metadata = file
            .metadata()
            .map_err(|err| AppError::io("initialize", error_code, err))?;
        if is_link_or_reparse_point(&metadata) || !metadata.is_file() {
            return Err(AppError::new(
                "workspace_managed_file_link_rejected",
                "工作区受管文件不能是符号链接、reparse point 或目录。",
                "workspace_boundary",
                false,
            )
            .with_details(candidate.to_string_lossy().to_string()));
        }
        let resolved = fs::canonicalize(&candidate)
            .map_err(|err| AppError::io("initialize", error_code, err))?;
        if !resolved.starts_with(&parent) {
            return Err(AppError::new(
                "workspace_path_outside_root",
                "工作区受管文件越界，已拒绝文件操作。",
                "workspace_boundary",
                false,
            )
            .with_details(resolved.to_string_lossy().to_string()));
        }
        Ok(())
    }

    fn required_dirs(&self) -> [PathBuf; 11] {
        [
            self.originals_dir(),
            self.pages_dir(),
            self.analysis_dir(),
            self.canonical_pdfs_dir(),
            self.pdf_structure_dir(),
            self.metadata_dir(),
            self.indexes_dir(),
            self.bm25_index_dir(),
            self.jobs_dir(),
            self.logs_dir(),
            self.tmp_dir(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::WorkspaceLayout;
    use std::fs;

    #[test]
    fn initializes_base_layout_idempotently() {
        let root =
            std::env::temp_dir().join(format!("slicer-layout-测试 工作区-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temp root");

        let layout = WorkspaceLayout::from_root(root.clone());
        layout.ensure_base_layout().expect("first init");
        fs::write(layout.originals_dir().join("keep.txt"), "keep").expect("sentinel");
        fs::write(layout.app_db_path(), "existing").expect("existing db");

        layout.ensure_base_layout().expect("second init");

        for path in [
            layout.originals_dir(),
            layout.pages_dir(),
            layout.analysis_dir(),
            layout.canonical_pdfs_dir(),
            layout.pdf_structure_dir(),
            layout.metadata_dir(),
            layout.indexes_dir(),
            layout.bm25_index_dir(),
            layout.jobs_dir(),
            layout.logs_dir(),
            layout.tmp_dir(),
        ] {
            assert!(path.is_dir(), "{path:?} should exist");
        }
        assert_eq!(
            fs::read_to_string(layout.originals_dir().join("keep.txt")).expect("sentinel"),
            "keep"
        );
        assert_eq!(
            fs::read_to_string(layout.app_db_path()).expect("db"),
            "existing"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_path_like_storage_ids_before_joining_them() {
        let root = std::env::temp_dir().join(format!(
            "slicer-layout-storage-id-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("temp root");
        let layout = WorkspaceLayout::from_root(root.clone());
        layout.ensure_base_layout().expect("layout");

        for invalid in ["../outside", "..\\outside", "not-a-uuid", ""] {
            let error = layout
                .ensure_managed_document_dir(&layout.canonical_pdfs_dir(), invalid)
                .expect_err("invalid id should be rejected");
            assert_eq!(error.code, "workspace_storage_id_invalid");
        }

        assert!(!root.join("outside").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn validates_each_layout_component_before_creating_children() {
        let root = std::env::temp_dir().join(format!(
            "slicer-layout-component-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("temp root");
        fs::write(root.join("indexes"), "not a directory").expect("blocking leaf");
        let layout = WorkspaceLayout::from_root(root.clone());

        let error = layout
            .ensure_base_layout()
            .expect_err("file in directory position must be rejected");
        assert_eq!(error.code, "workspace_managed_dir_link_rejected");
        assert!(!root.join("indexes").join("bm25").exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_directory_in_place_of_managed_ledger_file() {
        let root = std::env::temp_dir().join(format!(
            "slicer-layout-ledger-leaf-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(root.join("app.db")).expect("fake ledger directory");
        let layout = WorkspaceLayout::from_root(root.clone());

        let error = layout
            .ensure_base_layout()
            .expect_err("managed ledger must be a regular file");
        assert_eq!(error.code, "workspace_managed_file_link_rejected");

        let _ = fs::remove_dir_all(root);
    }
}
