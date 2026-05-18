use crate::errors::{AppError, AppResult};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

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

    pub fn ensure_base_layout(&self) -> AppResult<()> {
        for dir in self.required_dirs() {
            fs::create_dir_all(&dir)
                .map_err(|err| AppError::io("initialize", "workspace_create_dir_failed", err))?;
        }

        OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.app_db_path())
            .map_err(|err| AppError::io("initialize", "workspace_app_db_failed", err))?;

        OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.pages_jsonl_path())
            .map_err(|err| AppError::io("initialize", "workspace_pages_jsonl_failed", err))?;

        Ok(())
    }

    fn required_dirs(&self) -> [PathBuf; 9] {
        [
            self.originals_dir(),
            self.pages_dir(),
            self.analysis_dir(),
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
}
