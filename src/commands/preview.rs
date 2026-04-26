use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use calamine::{Data, Reader, open_workbook_auto};
use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::platform::{self, ensure_exists};

const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "md", "markdown", "html", "htm", "css", "js", "mjs", "cjs", "ts", "tsx", "jsx", "json",
    "xml", "yaml", "yml", "toml", "ini", "conf", "cfg", "log", "csv", "tsv", "rs", "py", "rb",
    "go", "java", "kt", "swift", "c", "cpp", "cc", "cxx", "h", "hpp", "hh", "sh", "bash", "zsh",
    "fish", "bat", "ps1", "psm1", "sql", "lua", "pl", "php", "vue", "svelte", "scss", "sass",
    "less", "env", "gitignore", "gitattributes", "lock", "tex", "diff", "patch",
];

const IMAGE_EXTENSIONS: &[&str] =
    &["png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico", "tiff", "tif"];

const TEXT_PREVIEW_LIMIT: u64 = 10 * 1024;
const TEXT_SNIFF_LIMIT: u64 = 64 * 1024;
const IMAGE_PREVIEW_LIMIT: u64 = 20 * 1024 * 1024;

const XLSX_LIMIT: u64 = 50 * 1024 * 1024;
const PDF_LIMIT: u64 = 100 * 1024 * 1024;
const DOCX_LIMIT: u64 = 20 * 1024 * 1024;
const PPTX_LIMIT: u64 = 50 * 1024 * 1024;

const SPREADSHEET_MAX_ROWS: usize = 100;
const SPREADSHEET_MAX_COLS: usize = 20;
const PPTX_CONVERT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PreviewPayload {
    #[serde(rename_all = "camelCase")]
    Directory {
        sub_dirs: u64,
        sub_files: u64,
        total_size: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Text {
        content: String,
        truncated: bool,
        total_size: u64,
    },
    #[serde(rename_all = "camelCase")]
    Image { path: String, size: u64 },
    #[serde(rename_all = "camelCase")]
    ImageTooLarge { size: u64 },
    #[serde(rename_all = "camelCase")]
    Spreadsheet {
        sheet_name: String,
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        total_rows: u32,
        total_cols: u32,
        truncated_rows: bool,
        truncated_cols: bool,
        other_sheets: Vec<String>,
        size: u64,
    },
    #[serde(rename_all = "camelCase")]
    Pdf { path: String, size: u64 },
    #[serde(rename_all = "camelCase")]
    Docx { path: String, size: u64 },
    #[serde(rename_all = "camelCase")]
    Pptx { pdf_path: String, size: u64 },
    #[serde(rename_all = "camelCase")]
    OfficeImage {
        image_path: String,
        size: u64,
        engine: String,
    },
    #[serde(rename_all = "camelCase")]
    Unsupported { reason: String, size: u64 },
    #[serde(rename_all = "camelCase")]
    Other { size: u64 },
}

#[tauri::command]
pub async fn preview(path: String) -> AppResult<PreviewPayload> {
    tokio::task::spawn_blocking(move || preview_blocking(path))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

fn preview_blocking(path: String) -> AppResult<PreviewPayload> {
    let p = PathBuf::from(&path);
    ensure_exists(&p)?;
    let meta = std::fs::metadata(&p).map_err(AppError::from)?;
    if meta.is_dir() {
        return preview_dir(&p);
    }
    let size = meta.len();
    if is_image(&p) {
        if size > IMAGE_PREVIEW_LIMIT {
            return Ok(PreviewPayload::ImageTooLarge { size });
        }
        return Ok(PreviewPayload::Image {
            path: platform::path_to_string(&p),
            size,
        });
    }
    if let Some(ext) = ext_lower(&p) {
        match ext.as_str() {
            "xlsx" | "pdf" | "docx" | "pptx" => {
                // macOS 上优先试 Quick Look（装了 MS Office 后由 Office 渲染，
                // 不依赖 LibreOffice / PDF.js / mammoth）。
                #[cfg(target_os = "macos")]
                {
                    let limit = match ext.as_str() {
                        "xlsx" => XLSX_LIMIT,
                        "pdf" => PDF_LIMIT,
                        "docx" => DOCX_LIMIT,
                        "pptx" => PPTX_LIMIT,
                        _ => unreachable!(),
                    };
                    if size <= limit {
                        if let Some(payload) = try_quicklook(&p, size, &meta) {
                            return Ok(payload);
                        }
                    }
                }
                return match ext.as_str() {
                    "xlsx" => preview_xlsx(&p, size),
                    "pdf" => preview_pdf(&p, size),
                    "docx" => preview_docx(&p, size),
                    "pptx" => preview_pptx(&p, size, &meta),
                    _ => unreachable!(),
                };
            }
            _ => {}
        }
    }
    if is_text(&p) {
        return preview_text(&p, size);
    }
    Ok(PreviewPayload::Other { size })
}

fn preview_dir(p: &Path) -> AppResult<PreviewPayload> {
    let mut sub_dirs: u64 = 0;
    let mut sub_files: u64 = 0;
    let read = std::fs::read_dir(p).map_err(AppError::from)?;
    for entry in read.flatten() {
        match entry.file_type() {
            Ok(ft) if ft.is_dir() => sub_dirs += 1,
            Ok(_) => sub_files += 1,
            Err(_) => {}
        }
    }

    let total_size = du_size(p);
    Ok(PreviewPayload::Directory {
        sub_dirs,
        sub_files,
        total_size,
    })
}

fn du_size(p: &Path) -> Option<u64> {
    if cfg!(target_os = "windows") {
        return None;
    }
    if !platform::has_command("du") {
        return None;
    }
    let output = Command::new("du").arg("-sk").arg(p).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first = stdout.split_whitespace().next()?;
    let kb: u64 = first.parse().ok()?;
    Some(kb.saturating_mul(1024))
}

fn preview_text(p: &Path, size: u64) -> AppResult<PreviewPayload> {
    use std::io::Read;
    // 嗅探读取（最多 64KB）：chardetng 对 CJK 短样本的判断不稳，需要更大的样本。
    let sniff_len = TEXT_SNIFF_LIMIT.min(size);
    let mut f = std::fs::File::open(p).map_err(AppError::from)?;
    let mut sniff_buf = Vec::with_capacity(sniff_len as usize);
    f.by_ref()
        .take(sniff_len)
        .read_to_end(&mut sniff_buf)
        .map_err(AppError::from)?;

    // 预览实际只展示前 TEXT_PREVIEW_LIMIT 字节。
    let preview_len = TEXT_PREVIEW_LIMIT.min(size) as usize;
    let preview_bytes = &sniff_buf[..preview_len.min(sniff_buf.len())];
    let truncated = size > TEXT_PREVIEW_LIMIT;
    let content = decode_text(preview_bytes, &sniff_buf);
    Ok(PreviewPayload::Text {
        content,
        truncated,
        total_size: size,
    })
}

/// 解码顺序：
///   1. BOM (UTF-8 / UTF-16LE / UTF-16BE)
///   2. UTF-8 试解码
///   3. CJK 常见编码：GB18030 → Big5 → Shift_JIS → EUC-KR
///   4. chardetng 最后兜底
///
/// 每一步用嗅探样本（最多 64KB）"无解码错误"来判定命中。
/// 由于按字节读取可能在多字节字符中间截断，每一次尝试都会再裁掉末尾最多 4 字节重试。
fn decode_text(buf: &[u8], sniff: &[u8]) -> String {
    // 1) BOM
    if sniff.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let start = if buf.starts_with(&[0xEF, 0xBB, 0xBF]) {
            3
        } else {
            0
        };
        let (cow, _) = encoding_rs::UTF_8.decode_without_bom_handling(&buf[start..]);
        return cow.into_owned();
    }
    if sniff.starts_with(&[0xFF, 0xFE]) {
        let start = if buf.starts_with(&[0xFF, 0xFE]) { 2 } else { 0 };
        let (cow, _) = encoding_rs::UTF_16LE.decode_without_bom_handling(&buf[start..]);
        return cow.into_owned();
    }
    if sniff.starts_with(&[0xFE, 0xFF]) {
        let start = if buf.starts_with(&[0xFE, 0xFF]) { 2 } else { 0 };
        let (cow, _) = encoding_rs::UTF_16BE.decode_without_bom_handling(&buf[start..]);
        return cow.into_owned();
    }

    // 2 + 3) 顺序尝试 UTF-8 / GB18030 / Big5 / Shift_JIS / EUC-KR
    const CANDIDATES: &[&encoding_rs::Encoding] = &[
        encoding_rs::UTF_8,
        encoding_rs::GB18030,
        encoding_rs::BIG5,
        encoding_rs::SHIFT_JIS,
        encoding_rs::EUC_KR,
    ];
    for enc in CANDIDATES {
        if decodes_cleanly(enc, sniff) {
            let (cow, _) = enc.decode_without_bom_handling(buf);
            return cow.into_owned();
        }
    }

    // 4) chardetng 兜底
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(sniff, true);
    let encoding = detector.guess(None, true);
    let (cow, _) = encoding.decode_without_bom_handling(buf);
    cow.into_owned()
}

/// 尝试用指定编码解码嗅探样本；若末尾 1~4 个字节可能是被截断的多字节字符，
/// 逐步裁掉重试。任意一种裁剪长度下解码无错误则视为命中。
fn decodes_cleanly(enc: &'static encoding_rs::Encoding, sniff: &[u8]) -> bool {
    let max_trim = 4.min(sniff.len());
    for trim in 0..=max_trim {
        let slice = &sniff[..sniff.len() - trim];
        let (_, had_errors) = enc.decode_without_bom_handling(slice);
        if !had_errors {
            return true;
        }
    }
    false
}

fn ext_lower(p: &Path) -> Option<String> {
    p.extension().map(|e| e.to_string_lossy().to_lowercase())
}

fn is_image(p: &Path) -> bool {
    matches!(ext_lower(p), Some(ref e) if IMAGE_EXTENSIONS.contains(&e.as_str()))
}

fn is_text(p: &Path) -> bool {
    if let Some(ref e) = ext_lower(p) {
        if TEXT_EXTENSIONS.contains(&e.as_str()) {
            return true;
        }
    }
    if cfg!(unix) && platform::has_command("file") {
        if let Ok(output) = Command::new("file").arg("--mime").arg(p).output() {
            if output.status.success() {
                let s = String::from_utf8_lossy(&output.stdout);
                return s.contains("text/")
                    || s.contains("application/json")
                    || s.contains("application/xml");
            }
        }
    }
    false
}

// -----------------------------------------------------------------------------
// Office / PDF preview
// -----------------------------------------------------------------------------

fn too_large(size: u64, limit: u64, kind: &str) -> PreviewPayload {
    PreviewPayload::Unsupported {
        reason: format!(
            "{} 文件大小 {} 超过预览上限 {}",
            kind,
            human_bytes(size),
            human_bytes(limit)
        ),
        size,
    }
}

fn human_bytes(n: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", n, UNITS[0])
    } else {
        format!("{:.2} {}", v, UNITS[i])
    }
}

fn preview_pdf(p: &Path, size: u64) -> AppResult<PreviewPayload> {
    if size > PDF_LIMIT {
        return Ok(too_large(size, PDF_LIMIT, "PDF"));
    }
    Ok(PreviewPayload::Pdf {
        path: platform::path_to_string(p),
        size,
    })
}

fn preview_docx(p: &Path, size: u64) -> AppResult<PreviewPayload> {
    if size > DOCX_LIMIT {
        return Ok(too_large(size, DOCX_LIMIT, "DOCX"));
    }
    Ok(PreviewPayload::Docx {
        path: platform::path_to_string(p),
        size,
    })
}

fn preview_xlsx(p: &Path, size: u64) -> AppResult<PreviewPayload> {
    if size > XLSX_LIMIT {
        return Ok(too_large(size, XLSX_LIMIT, "XLSX"));
    }
    let mut wb = match open_workbook_auto(p) {
        Ok(w) => w,
        Err(e) => {
            return Ok(PreviewPayload::Unsupported {
                reason: format!("无法解析 XLSX：{}", e),
                size,
            });
        }
    };
    let names = wb.sheet_names();
    if names.is_empty() {
        return Ok(PreviewPayload::Unsupported {
            reason: "XLSX 文件不包含任何工作表".into(),
            size,
        });
    }
    let sheet_name = names[0].clone();
    let other_sheets: Vec<String> = names.iter().skip(1).cloned().collect();
    let range = match wb.worksheet_range(&sheet_name) {
        Ok(r) => r,
        Err(e) => {
            return Ok(PreviewPayload::Unsupported {
                reason: format!("无法读取工作表「{}」：{}", sheet_name, e),
                size,
            });
        }
    };

    let (h, w) = range.get_size();
    let total_rows = h as u32;
    let total_cols = w as u32;
    let truncated_rows = h > SPREADSHEET_MAX_ROWS;
    let truncated_cols = w > SPREADSHEET_MAX_COLS;

    let mut iter = range.rows().take(SPREADSHEET_MAX_ROWS);
    let headers: Vec<String> = iter
        .next()
        .map(|row| {
            row.iter()
                .take(SPREADSHEET_MAX_COLS)
                .map(cell_to_string)
                .collect()
        })
        .unwrap_or_default();
    let rows: Vec<Vec<String>> = iter
        .map(|row| {
            row.iter()
                .take(SPREADSHEET_MAX_COLS)
                .map(cell_to_string)
                .collect()
        })
        .collect();

    Ok(PreviewPayload::Spreadsheet {
        sheet_name,
        headers,
        rows,
        total_rows,
        total_cols,
        truncated_rows,
        truncated_cols,
        other_sheets,
        size,
    })
}

fn cell_to_string(d: &Data) -> String {
    match d {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Bool(b) => b.to_string(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => {
            if f.fract() == 0.0 && f.is_finite() && f.abs() < 1e16 {
                format!("{}", *f as i64)
            } else {
                format!("{}", f)
            }
        }
        Data::DateTime(dt) => dt.to_string(),
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("#{:?}", e),
    }
}

// -----------------------------------------------------------------------------
// PPTX → PDF via LibreOffice
// -----------------------------------------------------------------------------

fn preview_pptx(p: &Path, size: u64, meta: &fs::Metadata) -> AppResult<PreviewPayload> {
    if size > PPTX_LIMIT {
        return Ok(too_large(size, PPTX_LIMIT, "PPTX"));
    }
    let cache_dir = std::env::temp_dir().join("qfinder-preview");
    if let Err(e) = fs::create_dir_all(&cache_dir) {
        return Ok(PreviewPayload::Unsupported {
            reason: format!("无法创建缓存目录：{}", e),
            size,
        });
    }
    let cache_key = cache_key_for(p, size, meta);

    if let Some(soffice) = find_soffice() {
        return convert_pptx_via_soffice(p, size, &soffice, &cache_dir, &cache_key);
    }
    // 注：macOS 上的 Quick Look 兜底已在 preview_blocking 入口处优先尝试过；
    // 这里仅在 Quick Look 不可用或失败后抵达，返回 Unsupported。
    Ok(PreviewPayload::Unsupported {
        reason: "需要安装 LibreOffice 才能预览 .pptx 文件".into(),
        size,
    })
}

fn cache_key_for(p: &Path, size: u64, meta: &fs::Metadata) -> String {
    let mtime_secs = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path_str = platform::path_to_string(p);
    format!(
        "{:x}-{}-{}",
        djb2_hash(path_str.as_bytes()),
        mtime_secs,
        size
    )
}

fn convert_pptx_via_soffice(
    p: &Path,
    size: u64,
    soffice: &Path,
    cache_dir: &Path,
    key: &str,
) -> AppResult<PreviewPayload> {
    let cached_pdf = cache_dir.join(format!("{}.pdf", key));
    if cached_pdf.exists() {
        return Ok(PreviewPayload::Pptx {
            pdf_path: platform::path_to_string(&cached_pdf),
            size,
        });
    }

    // 在每次转换的隔离 user profile 下运行，避免与已开启的 LibreOffice 冲突。
    let work_dir = cache_dir.join(format!("{}.work", key));
    let _ = fs::remove_dir_all(&work_dir);
    if let Err(e) = fs::create_dir_all(&work_dir) {
        return Ok(PreviewPayload::Unsupported {
            reason: format!("无法创建工作目录：{}", e),
            size,
        });
    }
    let user_profile = work_dir.join("profile");
    let user_profile_arg = format!(
        "-env:UserInstallation=file://{}",
        user_profile.to_string_lossy()
    );

    let mut cmd = Command::new(soffice);
    cmd.arg(&user_profile_arg)
        .arg("--headless")
        .arg("--norestore")
        .arg("--nofirststartwizard")
        .arg("--convert-to")
        .arg("pdf")
        .arg("--outdir")
        .arg(&work_dir)
        .arg(p);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return Ok(PreviewPayload::Unsupported {
                reason: format!("启动 LibreOffice 失败：{}", e),
                size,
            });
        }
    };

    let started = Instant::now();
    let exit_status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if started.elapsed() > PPTX_CONVERT_TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };

    if exit_status.map(|s| !s.success()).unwrap_or(true) {
        let _ = fs::remove_dir_all(&work_dir);
        return Ok(PreviewPayload::Unsupported {
            reason: "LibreOffice 转换失败或超时".into(),
            size,
        });
    }

    // soffice 输出文件名 = <basename>.pdf
    let stem = p.file_stem().map(|s| s.to_os_string()).unwrap_or_default();
    let mut produced = work_dir.clone();
    produced.push(format!("{}.pdf", stem.to_string_lossy()));
    if !produced.exists() {
        let _ = fs::remove_dir_all(&work_dir);
        return Ok(PreviewPayload::Unsupported {
            reason: "LibreOffice 未生成 PDF 输出".into(),
            size,
        });
    }
    if let Err(e) = fs::rename(&produced, &cached_pdf) {
        // 跨设备时退化为 copy
        if fs::copy(&produced, &cached_pdf).is_err() {
            let _ = fs::remove_dir_all(&work_dir);
            return Ok(PreviewPayload::Unsupported {
                reason: format!("无法保存转换结果：{}", e),
                size,
            });
        }
    }
    let _ = fs::remove_dir_all(&work_dir);

    Ok(PreviewPayload::Pptx {
        pdf_path: platform::path_to_string(&cached_pdf),
        size,
    })
}

#[cfg(target_os = "macos")]
fn try_quicklook(p: &Path, size: u64, meta: &fs::Metadata) -> Option<PreviewPayload> {
    if !platform::has_command("qlmanage") {
        return None;
    }
    let cache_dir = std::env::temp_dir().join("qfinder-preview");
    fs::create_dir_all(&cache_dir).ok()?;
    let key = cache_key_for(p, size, meta);
    let cached_png = cache_dir.join(format!("{}.png", key));
    let engine = "macOS Quick Look".to_string();

    if cached_png.exists() {
        return Some(PreviewPayload::OfficeImage {
            image_path: platform::path_to_string(&cached_png),
            size,
            engine,
        });
    }

    let work_dir = cache_dir.join(format!("{}.qlwork", key));
    let _ = fs::remove_dir_all(&work_dir);
    fs::create_dir_all(&work_dir).ok()?;

    // qlmanage -t -s 1600 -o <work_dir> <file>
    let mut cmd = Command::new("qlmanage");
    cmd.arg("-t")
        .arg("-s")
        .arg("1600")
        .arg("-o")
        .arg(&work_dir)
        .arg(p);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => {
            let _ = fs::remove_dir_all(&work_dir);
            return None;
        }
    };

    let started = Instant::now();
    let exit_status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if started.elapsed() > PPTX_CONVERT_TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };

    if exit_status.map(|s| !s.success()).unwrap_or(true) {
        let _ = fs::remove_dir_all(&work_dir);
        return None;
    }

    // 输出文件名候选：<basename>.png（含原扩展名）/ <stem>.png / 任意 png。
    let file_name = p
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut produced = None;
    for cand in [
        work_dir.join(format!("{}.png", file_name)),
        work_dir.join(format!("{}.png", stem)),
    ] {
        if cand.exists() {
            produced = Some(cand);
            break;
        }
    }
    if produced.is_none() {
        produced = fs::read_dir(&work_dir).ok().and_then(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .find(|p| p.extension().and_then(|s| s.to_str()) == Some("png"))
        });
    }
    let Some(produced) = produced else {
        let _ = fs::remove_dir_all(&work_dir);
        return None;
    };

    if fs::rename(&produced, &cached_png).is_err() && fs::copy(&produced, &cached_png).is_err() {
        let _ = fs::remove_dir_all(&work_dir);
        return None;
    }
    let _ = fs::remove_dir_all(&work_dir);

    Some(PreviewPayload::OfficeImage {
        image_path: platform::path_to_string(&cached_png),
        size,
        engine,
    })
}

fn find_soffice() -> Option<PathBuf> {
    if let Ok(p) = which::which("soffice") {
        return Some(p);
    }
    if let Ok(p) = which::which("libreoffice") {
        return Some(p);
    }
    let mac = PathBuf::from("/Applications/LibreOffice.app/Contents/MacOS/soffice");
    if mac.exists() {
        return Some(mac);
    }
    None
}

fn djb2_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for b in data {
        h = h.wrapping_mul(33).wrapping_add(*b as u64);
    }
    h
}
