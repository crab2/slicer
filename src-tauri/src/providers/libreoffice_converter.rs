use crate::errors::{AppError, AppResult};
use crate::providers::converter::DocumentConverter;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct LibreOfficeConverter {
    libreoffice_path: String,
}

impl LibreOfficeConverter {
    pub fn new(libreoffice_path: String) -> Self {
        Self { libreoffice_path }
    }
}

impl DocumentConverter for LibreOfficeConverter {
    fn convert_to_pdf(&self, input_path: &Path, output_dir: &Path) -> AppResult<PathBuf> {
        let executable = resolve_libreoffice_executable(&self.libreoffice_path)?;

        let input_abs = input_path.canonicalize().map_err(|e| {
            AppError::new(
                "input_path_invalid",
                "无法访问输入文件路径。",
                "conversion",
                true,
            )
            .with_details(e.to_string())
        })?;

        let output_abs = output_dir.canonicalize().map_err(|e| {
            AppError::new(
                "output_dir_invalid",
                "无法访问输出目录路径。",
                "conversion",
                true,
            )
            .with_details(e.to_string())
        })?;

        let output = Command::new(&executable)
            .arg("--headless")
            .arg("--convert-to")
            .arg("pdf")
            .arg("--outdir")
            .arg(&output_abs)
            .arg(&input_abs)
            .output()
            .map_err(|e| {
                AppError::new(
                    "libreoffice_exec_failed",
                    "无法启动 LibreOffice 进程，请检查路径配置。",
                    "conversion",
                    true,
                )
                .with_details(format!(
                    "executable: {}; error: {}",
                    executable.display(),
                    e
                ))
            })?;

        if !output.status.success() {
            return Err(AppError::new(
                "libreoffice_convert_failed",
                "LibreOffice 转换失败，文件可能已损坏或格式不受支持。",
                "conversion",
                true,
            )
            .with_details(command_diagnostics(&executable, &output)));
        }

        let stem = input_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("converted");
        let pdf_path = output_abs.join(format!("{stem}.pdf"));

        if !pdf_path.exists() {
            return Err(AppError::new(
                "libreoffice_output_missing",
                "LibreOffice 转换完成但未找到输出 PDF 文件。",
                "conversion",
                true,
            )
            .with_details(format!(
                "expected_path: {}; {}",
                pdf_path.display(),
                command_diagnostics(&executable, &output)
            )));
        }

        Ok(pdf_path)
    }
}

fn resolve_libreoffice_executable(configured_path: &str) -> AppResult<PathBuf> {
    let trimmed = configured_path
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim();

    if trimmed.is_empty() {
        return Err(AppError::new(
            "libreoffice_not_configured",
            "请在设置中配置 LibreOffice 路径后再导入 Office 文档。",
            "conversion",
            true,
        ));
    }

    let path = PathBuf::from(trimmed);
    if path.is_file() {
        return Ok(path);
    }

    if path.is_dir() {
        for executable_name in libreoffice_executable_names() {
            let candidate = path.join(executable_name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }

        return Err(AppError::new(
            "libreoffice_executable_not_found",
            "LibreOffice 路径不包含可执行文件，请检查配置。",
            "conversion",
            true,
        )
        .with_details(format!(
            "configured_path: {}; tried: {}",
            path.display(),
            libreoffice_executable_names().join(", ")
        )));
    }

    #[cfg(windows)]
    {
        if path.extension().is_none() {
            for extension in ["com", "exe"] {
                let candidate = path.with_extension(extension);
                if candidate.is_file() {
                    return Ok(candidate);
                }
            }
        }
    }

    Err(AppError::new(
        "libreoffice_not_found",
        "LibreOffice 路径不存在，请在设置中检查配置。",
        "conversion",
        true,
    )
    .with_details(format!("configured_path: {}", configured_path)))
}

pub fn find_libreoffice_installation() -> Option<PathBuf> {
    find_libreoffice_in_candidates(libreoffice_search_candidates())
}

fn find_libreoffice_in_candidates(candidates: Vec<PathBuf>) -> Option<PathBuf> {
    let mut seen = HashSet::new();

    for candidate in candidates {
        let key = normalize_candidate_key(&candidate);
        if !seen.insert(key) {
            continue;
        }

        if let Some(path) = libreoffice_candidate_config_path(&candidate) {
            return Some(path);
        }
    }

    None
}

fn libreoffice_candidate_config_path(candidate: &Path) -> Option<PathBuf> {
    if candidate.is_file() && is_libreoffice_executable(candidate) {
        return Some(candidate.to_path_buf());
    }

    if candidate.is_dir() {
        for executable_name in libreoffice_executable_names() {
            if candidate.join(executable_name).is_file() {
                return Some(candidate.to_path_buf());
            }
        }
    }

    None
}

fn libreoffice_search_candidates() -> Vec<PathBuf> {
    let mut candidates = standard_libreoffice_search_dirs();

    if let Some(path_var) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path_var));
    }

    candidates
}

#[cfg(windows)]
fn standard_libreoffice_search_dirs() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    for env_key in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
        if let Some(root) = std::env::var_os(env_key) {
            candidates.push(PathBuf::from(root).join("LibreOffice").join("program"));
        }
    }

    candidates.push(PathBuf::from(r"C:\Program Files\LibreOffice\program"));
    candidates.push(PathBuf::from(r"C:\Program Files (x86)\LibreOffice\program"));
    candidates
}

#[cfg(target_os = "macos")]
fn standard_libreoffice_search_dirs() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/Applications/LibreOffice.app/Contents/MacOS"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/opt/homebrew/bin"),
    ]
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn standard_libreoffice_search_dirs() -> Vec<PathBuf> {
    let mut candidates = vec![
        PathBuf::from("/usr/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/opt/libreoffice/program"),
        PathBuf::from("/snap/bin"),
    ];

    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(
            PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("flatpak")
                .join("exports")
                .join("bin"),
        );
    }

    candidates
}

fn is_libreoffice_executable(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    libreoffice_executable_names()
        .iter()
        .any(|name| file_name.eq_ignore_ascii_case(name))
}

#[cfg(windows)]
fn normalize_candidate_key(path: &Path) -> String {
    path.to_string_lossy().to_ascii_lowercase()
}

#[cfg(not(windows))]
fn normalize_candidate_key(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(windows)]
fn libreoffice_executable_names() -> &'static [&'static str] {
    &["soffice.com", "soffice.exe", "libreoffice.exe"]
}

#[cfg(not(windows))]
fn libreoffice_executable_names() -> &'static [&'static str] {
    &["soffice", "libreoffice"]
}

fn command_diagnostics(executable: &Path, output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!(
        "executable: {}; exit_code: {:?}; stdout: {}; stderr: {}",
        executable.display(),
        output.status.code(),
        compact_process_text(&stdout),
        compact_process_text(&stderr)
    )
}

fn compact_process_text(text: &str) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        "<empty>".to_string()
    } else {
        compact.chars().take(400).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{find_libreoffice_in_candidates, resolve_libreoffice_executable};
    use std::fs;
    use std::path::{Path, PathBuf};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "slicer-lo-converter-{name}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn create_launcher(dir: &Path) -> PathBuf {
        #[cfg(windows)]
        let launcher = dir.join("soffice.com");
        #[cfg(not(windows))]
        let launcher = dir.join("soffice");

        fs::write(&launcher, b"").expect("launcher file");
        launcher
    }

    #[test]
    fn resolves_configured_program_directory_to_launcher() {
        let dir = unique_temp_dir("directory");
        fs::create_dir_all(&dir).expect("temp dir");
        let launcher = create_launcher(&dir);

        let resolved =
            resolve_libreoffice_executable(dir.to_str().expect("utf8 path")).expect("resolved");

        assert_eq!(resolved, launcher);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn resolves_quoted_executable_path() {
        let dir = unique_temp_dir("quoted");
        fs::create_dir_all(&dir).expect("temp dir");
        let launcher = create_launcher(&dir);
        let configured = format!("\"{}\"", launcher.display());

        let resolved = resolve_libreoffice_executable(&configured).expect("resolved");

        assert_eq!(resolved, launcher);
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(windows)]
    #[test]
    fn resolves_extensionless_windows_executable_path() {
        let dir = unique_temp_dir("extensionless");
        fs::create_dir_all(&dir).expect("temp dir");
        let launcher = dir.join("soffice.exe");
        fs::write(&launcher, b"").expect("launcher file");
        let configured = dir.join("soffice");

        let resolved = resolve_libreoffice_executable(configured.to_str().expect("utf8 path"))
            .expect("resolved");

        assert_eq!(resolved, launcher);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_directory_without_launcher() {
        let dir = unique_temp_dir("missing");
        fs::create_dir_all(&dir).expect("temp dir");

        let err =
            resolve_libreoffice_executable(dir.to_str().expect("utf8 path")).expect_err("error");

        assert_eq!(err.code, "libreoffice_executable_not_found");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn finds_configurable_program_directory() {
        let dir = unique_temp_dir("find-directory");
        fs::create_dir_all(&dir).expect("temp dir");
        create_launcher(&dir);

        let found = find_libreoffice_in_candidates(vec![dir.clone()]).expect("found");

        assert_eq!(found, dir);
        let _ = fs::remove_dir_all(found);
    }

    #[test]
    fn finds_direct_executable_candidate() {
        let dir = unique_temp_dir("find-executable");
        fs::create_dir_all(&dir).expect("temp dir");
        let launcher = create_launcher(&dir);

        let found = find_libreoffice_in_candidates(vec![launcher.clone()]).expect("found");

        assert_eq!(found, launcher);
        let _ = fs::remove_dir_all(dir);
    }
}
