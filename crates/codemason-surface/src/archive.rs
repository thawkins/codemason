//! Bounded zip / tar.gz extraction shared by `/plugins` and `/connectors`
//! (RS-10).
//!
//! Both crates carried the same extraction loops. They now share this one,
//! which returns the neutral [`ExtractError`]; each crate maps it into its own
//! error type, so its public messages are unchanged. Entry paths are checked
//! with [`codemason_types::guard::validate_relative_component`], the stricter
//! of the two former checks (it also refuses an empty name, a backslash and a
//! Windows drive or UNC prefix on every platform).
//!
//! Depends on `zip`, `flate2` and `tar` for the formats.

use std::io::{Cursor, Read as _};
use std::path::{Path, PathBuf};

use codemason_types::guard::validate_relative_component;

/// An archive format, chosen from the file name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    /// A `.zip` archive.
    Zip,
    /// A `.tar.gz` or `.tgz` archive.
    TarGz,
}

impl ArchiveKind {
    /// The format a path or URL names by its extension, case-insensitively.
    #[must_use]
    pub fn from_path(name: &str) -> Option<Self> {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".tar.gz") {
            return Some(Self::TarGz);
        }
        match Path::new(&lower).extension().and_then(|e| e.to_str()) {
            Some("zip") => Some(Self::Zip),
            Some("tgz") => Some(Self::TarGz),
            _ => None,
        }
    }
}

/// The caps one extraction enforces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtractLimits {
    /// Largest decompressed size of one entry, in bytes.
    pub max_entry_bytes: u64,
    /// Largest decompressed size of the whole archive, in bytes.
    pub max_total_bytes: u64,
    /// Most entries an archive may hold.
    pub max_entries: usize,
}

/// Why an extraction failed.
#[derive(Debug)]
pub enum ExtractError {
    /// An entry path would land outside the staging directory.
    UnsafePath(String),
    /// One entry decompressed past [`ExtractLimits::max_entry_bytes`].
    EntryTooLarge {
        /// `"zip"` or `"tar"`.
        format: &'static str,
        /// The entry's path inside the archive.
        name: String,
        /// Bytes read before the cap stopped the read.
        size: u64,
    },
    /// A filesystem error writing the staging tree.
    Io(std::io::Error),
    /// Any other failure, already worded for the user.
    Message(String),
}

/// Extract `bytes` into `staging`, enforcing `limits` and the entry-path
/// check.
///
/// # Errors
/// Returns [`ExtractError`] for a malformed archive, an unsafe entry path, a
/// cap breach or a write failure.
pub fn extract(
    bytes: &[u8],
    kind: ArchiveKind,
    staging: &Path,
    limits: ExtractLimits,
) -> Result<(), ExtractError> {
    match kind {
        ArchiveKind::Zip => extract_zip(bytes, staging, limits),
        ArchiveKind::TarGz => extract_tar_gz(bytes, staging, limits),
    }
}

/// Normalise an archive entry name and refuse one that escapes the root.
///
/// # Errors
/// [`ExtractError::UnsafePath`] holding the original name.
pub fn safe_relative_path(name: &str) -> Result<PathBuf, ExtractError> {
    let normalised = name.replace('\\', "/");
    if validate_relative_component(&normalised, "archive entry").is_err() {
        return Err(ExtractError::UnsafePath(name.to_string()));
    }
    Ok(PathBuf::from(normalised))
}

fn extract_zip(bytes: &[u8], staging: &Path, limits: ExtractLimits) -> Result<(), ExtractError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| ExtractError::Message(format!("zip open: {e}")))?;
    // RS-05: bound the entry count; zero-byte entries cost nothing against the
    // byte caps below.
    if archive.len() > limits.max_entries {
        return Err(ExtractError::Message(format!(
            "archive has {} entries, over the {} entry cap",
            archive.len(),
            limits.max_entries
        )));
    }
    let mut total: u64 = 0;
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| ExtractError::Message(format!("zip entry {i}: {e}")))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry
            .name()
            .map_err(|e| ExtractError::Message(format!("zip entry {i} name: {e}")))?
            .to_string();
        let rel = safe_relative_path(&name)?;
        let body = read_entry(entry, "zip", &name, limits, &mut total)?;
        write_entry(&staging.join(rel), &body)?;
    }
    Ok(())
}

fn extract_tar_gz(bytes: &[u8], staging: &Path, limits: ExtractLimits) -> Result<(), ExtractError> {
    let decoder = flate2::read::GzDecoder::new(Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|e| ExtractError::Message(format!("tar entries: {e}")))?;
    let mut total: u64 = 0;
    for (count, entry) in entries.enumerate() {
        // RS-05: bound the entry count; zero-byte entries cost nothing
        // against the byte caps below.
        if count >= limits.max_entries {
            return Err(ExtractError::Message(format!(
                "archive has more than {} entries",
                limits.max_entries
            )));
        }
        let entry = entry.map_err(|e| ExtractError::Message(format!("tar entry: {e}")))?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry
            .path()
            .map_err(|e| ExtractError::Message(format!("tar entry path: {e}")))?
            .to_path_buf();
        let name = path.to_string_lossy().replace('\\', "/");
        let rel = safe_relative_path(&name)?;
        let body = read_entry(entry, "tar", &name, limits, &mut total)?;
        write_entry(&staging.join(rel), &body)?;
    }
    Ok(())
}

/// Read one entry's body, bounding the read itself so a zip or gzip bomb is
/// never decompressed into memory before the size check runs (SEC-070,
/// ANTIPAT M17).
fn read_entry(
    entry: impl std::io::Read,
    format: &'static str,
    name: &str,
    limits: ExtractLimits,
    total: &mut u64,
) -> Result<Vec<u8>, ExtractError> {
    // Read at most the per-entry cap or the remaining total headroom, plus one
    // byte so an oversized entry is detected without reading all of it.
    let remaining = limits.max_total_bytes.saturating_sub(*total);
    let mut body = Vec::new();
    entry
        .take(limits.max_entry_bytes.min(remaining).saturating_add(1))
        .read_to_end(&mut body)
        .map_err(|e| ExtractError::Message(format!("{format} entry {name}: {e}")))?;
    if body.len() as u64 > limits.max_entry_bytes {
        return Err(ExtractError::EntryTooLarge {
            format,
            name: name.to_string(),
            size: body.len() as u64,
        });
    }
    *total = total.saturating_add(body.len() as u64);
    if *total > limits.max_total_bytes {
        return Err(ExtractError::Message(format!(
            "archive decompressed past the {} byte total cap",
            limits.max_total_bytes
        )));
    }
    Ok(body)
}

/// Write one entry to `dest`, creating its parent directories.
fn write_entry(dest: &Path, body: &[u8]) -> Result<(), ExtractError> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(ExtractError::Io)?;
    }
    std::fs::write(dest, body).map_err(ExtractError::Io)
}
