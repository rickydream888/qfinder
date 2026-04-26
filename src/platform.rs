use std::path::Path;

use crate::error::{AppError, AppResult};

/// Whether a path is hidden according to platform conventions.
///
/// On Windows, prefer passing the entry's own metadata (from
/// `DirEntry::metadata`) so reparse points are inspected directly rather than
/// following them to their target.
pub fn is_hidden(_path: &Path, name: &str, meta: Option<&std::fs::Metadata>) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
        if let Some(m) = meta {
            let attrs = m.file_attributes();
            if attrs & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0 {
                return true;
            }
        } else if let Ok(m) = std::fs::symlink_metadata(_path) {
            let attrs = m.file_attributes();
            if attrs & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0 {
                return true;
            }
        }
        name.starts_with('.')
    }
    #[cfg(not(windows))]
    {
        let _ = meta;
        name.starts_with('.')
    }
}

/// Windows backwards-compatibility junctions (e.g. `Application Data`,
/// `Local Settings`, `My Documents` under the user home). They are always
/// inaccessible to user processes and should never be listed.
pub fn is_compat_junction(_meta: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        let attrs = _meta.file_attributes();
        let mask = FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM | FILE_ATTRIBUTE_REPARSE_POINT;
        attrs & mask == mask
    }
    #[cfg(not(windows))]
    {
        false
    }
}

pub fn has_command(name: &str) -> bool {
    which::which(name).is_ok()
}

#[derive(serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RootEntry {
    pub label: String,
    pub path: String,
    pub kind: RootKind,
}

#[derive(serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // variants are constructed conditionally per OS
pub enum RootKind {
    Home,
    SystemRoot,
    Drive,
    Volume,
    ICloud,
    Removable,
}

pub fn list_roots() -> AppResult<Vec<RootEntry>> {
    let mut roots = Vec::new();

    if let Some(home) = dirs::home_dir() {
        let label = home
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path_to_string(&home));
        roots.push(RootEntry {
            label,
            path: path_to_string(&home),
            kind: RootKind::Home,
        });
    }

    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::GetLogicalDrives;
        let mask = unsafe { GetLogicalDrives() };
        for i in 0..26u32 {
            if mask & (1 << i) != 0 {
                let letter = (b'A' + i as u8) as char;
                let path = format!("{letter}:\\");
                if Path::new(&path).exists() {
                    roots.push(RootEntry {
                        label: format!("{letter}:"),
                        path,
                        kind: RootKind::Drive,
                    });
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        roots.push(RootEntry {
            label: "Macintosh HD".into(),
            path: "/".into(),
            kind: RootKind::SystemRoot,
        });
        if let Some(home) = dirs::home_dir() {
            let icloud = home.join("Library/Mobile Documents/com~apple~CloudDocs");
            if icloud.exists() {
                roots.push(RootEntry {
                    label: "iCloud Drive".into(),
                    path: path_to_string(&icloud),
                    kind: RootKind::ICloud,
                });
            }
        }
        if let Ok(read) = std::fs::read_dir("/Volumes") {
            for entry in read.flatten() {
                let p = entry.path();
                if !p.is_dir() {
                    continue;
                }
                // Skip the system root symlink (commonly named "Macintosh HD").
                if let Ok(target) = std::fs::read_link(&p) {
                    if target == Path::new("/") {
                        continue;
                    }
                }
                roots.push(RootEntry {
                    label: entry.file_name().to_string_lossy().into_owned(),
                    path: path_to_string(&p),
                    kind: RootKind::Volume,
                });
            }
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        roots.push(RootEntry {
            label: "/".into(),
            path: "/".into(),
            kind: RootKind::SystemRoot,
        });
        let user = std::env::var("USER").unwrap_or_default();
        let candidates = [
            format!("/media/{user}"),
            format!("/run/media/{user}"),
            "/mnt".to_string(),
            "/media".to_string(),
        ];
        for base in candidates {
            if let Ok(read) = std::fs::read_dir(&base) {
                for entry in read.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        roots.push(RootEntry {
                            label: entry.file_name().to_string_lossy().into_owned(),
                            path: path_to_string(&p),
                            kind: RootKind::Removable,
                        });
                    }
                }
            }
        }
    }

    Ok(roots)
}

pub fn path_to_string(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

pub fn ensure_exists(p: &Path) -> AppResult<()> {
    if !p.exists() {
        return Err(AppError::NotFound(path_to_string(p)));
    }
    Ok(())
}
