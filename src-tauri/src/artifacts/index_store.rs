use crate::domain::index::ActiveIndexPointer;
use crate::errors::{AppError, AppResult};
use std::fs;
use std::io::Write;
use std::path::Path;

pub fn read_active_pointer(path: &Path) -> AppResult<Option<ActiveIndexPointer>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)
        .map_err(|err| AppError::io("index", "active_pointer_read_failed", err))?;
    let pointer = serde_json::from_str(&raw).map_err(|err| {
        AppError::new(
            "active_pointer_invalid",
            "索引激活指针文件无效。",
            "index",
            false,
        )
        .with_details(err.to_string())
    })?;
    Ok(Some(pointer))
}

pub fn write_active_pointer(path: &Path, pointer: &ActiveIndexPointer) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| AppError::io("index", "active_pointer_dir_failed", err))?;
    }
    let tmp_path = path.with_extension("json.tmp");
    let payload = serde_json::to_string_pretty(pointer).map_err(|err| {
        AppError::new(
            "active_pointer_serialize_failed",
            "索引激活指针序列化失败。",
            "index",
            false,
        )
        .with_details(err.to_string())
    })?;
    let mut file = fs::File::create(&tmp_path)
        .map_err(|err| AppError::io("index", "active_pointer_tmp_failed", err))?;
    file.write_all(payload.as_bytes())
        .map_err(|err| AppError::io("index", "active_pointer_write_failed", err))?;
    fs::rename(&tmp_path, path)
        .map_err(|err| AppError::io("index", "active_pointer_rename_failed", err))?;
    Ok(())
}
