/************************************************
* File: archive.rs
* Author: Michal Švrček
*
* Portable ZIP export/import with safe paths and SHA-256 validation
*
* ver. 0.4.0
*************************************************/

use crate::{
    backup::{Entry, Manifest},
    config, output,
};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
use zip::{write::FileOptions, CompressionMethod, ZipArchive, ZipWriter};

const MAX_FILES: usize = 20_000;
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;

type ZipOptions = FileOptions<'static, ()>;

fn safe_path(value: &str) -> bool {
    !value.contains('\\')
        && !value.contains(':')
        && !value.is_empty()
        && Path::new(value)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

fn manifest_paths(manifest: &Manifest) -> Result<BTreeMap<String, Option<String>>, String> {
    config::valid_name(&manifest.profile)?;
    let mut paths = BTreeMap::new();
    if !manifest.entries.is_empty() {
        if manifest.entries.len() > MAX_FILES {
            return Err("Too many files in manifest".into());
        }
        for Entry {
            alias,
            relative,
            sha256,
        } in &manifest.entries
        {
            config::valid_name(alias)?;
            if !safe_path(relative)
                || !relative.starts_with(&format!("{alias}/"))
                || relative == &format!("{alias}/")
            {
                return Err(format!("Unsafe manifest entry: {relative}"));
            }
            if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(format!("Invalid hash: {relative}"));
            }
            let key = format!("data/{relative}");
            if paths
                .insert(key, Some(sha256.to_ascii_lowercase()))
                .is_some()
            {
                return Err("Duplicate manifest entry".into());
            }
        }
    } else {
        if manifest.files.len() > MAX_FILES {
            return Err("Too many files in legacy manifest".into());
        }
        for (alias, file) in &manifest.files {
            config::valid_name(alias)?;
            if file != &format!("{alias}.bak") {
                return Err("Unsafe legacy filename".into());
            }
            paths.insert(file.clone(), None);
        }
    }
    if paths.is_empty() {
        return Err("Empty backup manifest".into());
    }
    Ok(paths)
}

fn load_manifest(folder: &Path, profile: &str) -> Result<(Manifest, Vec<u8>), String> {
    let bytes = fs::read(folder.join("manifest.toml")).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err("Manifest too large".into());
    }
    let manifest: Manifest =
        toml::from_str(std::str::from_utf8(&bytes).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if manifest.profile != profile {
        return Err("Profile mismatch".into());
    }
    Ok((manifest, bytes))
}

fn digest<R: Read>(reader: &mut R, mut writer: impl Write) -> Result<(String, u64), String> {
    let mut hash = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 65536];
    loop {
        let n = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > MAX_FILE_BYTES {
            return Err("File exceeds 256 MiB limit".into());
        }
        hash.update(&buffer[..n]);
        writer.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
    }
    Ok((format!("{:x}", hash.finalize()), total))
}

pub fn export(profile: &str, id: Option<&str>, output_path: &Path) -> Result<(), String> {
    config::valid_name(profile)?;
    let folder = crate::backup::selected_folder(profile, id)?;
    let (manifest, bytes) = load_manifest(&folder, profile)?;
    let files = manifest_paths(&manifest)?;
    if output_path.exists() {
        return Err(format!("Refusing to overwrite: {}", output_path.display()));
    }
    let parent = output_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if !parent.is_dir() {
        return Err("Export destination directory does not exist".into());
    }
    let result = (|| -> Result<(), String> {
        let mut zip = ZipWriter::new(
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output_path)
                .map_err(|e| e.to_string())?,
        );
        let opts: ZipOptions = FileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o600);
        zip.start_file("manifest.toml", opts)
            .map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
        let mut all_bytes = 0u64;
        for (key, expected_hash) in files {
            let src = folder.join(&key);
            if fs::symlink_metadata(&src)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink()
            {
                return Err(format!("Symlink rejected: {}", src.display()));
            }
            let mut input = fs::File::open(&src).map_err(|e| e.to_string())?;
            zip.start_file(&key, opts).map_err(|e| e.to_string())?;
            let (hash, size) = digest(&mut input, &mut zip)?;
            all_bytes += size;
            if all_bytes > MAX_TOTAL_BYTES {
                return Err("Backup exceeds 1 GiB limit".into());
            }
            if let Some(expected) = expected_hash {
                if expected != hash {
                    return Err(format!("SHA-256 mismatch: {key}"));
                }
            }
        }
        zip.finish().map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(output_path);
    }
    result?;
    output::success(&format!("Exported: {}", output_path.display()));
    output::warn("ZIP is not encrypted. Keep it private and do not include credentials.");
    Ok(())
}

pub fn import(archive_path: &Path) -> Result<(), String> {
    if !archive_path.is_file() {
        return Err("ZIP file not found".into());
    }
    let file = fs::File::open(archive_path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Invalid ZIP: {e}"))?;
    if archive.len() == 0 || archive.len() > MAX_FILES + 1 {
        return Err("Invalid ZIP file count".into());
    }
    let mut manifest_bytes = Vec::new();
    {
        let mut entry = archive
            .by_name("manifest.toml")
            .map_err(|_| "Missing manifest.toml")?;
        if entry.size() > MAX_MANIFEST_BYTES {
            return Err("Manifest too large".into());
        }
        entry
            .read_to_end(&mut manifest_bytes)
            .map_err(|e| e.to_string())?;
    }
    let manifest: Manifest =
        toml::from_str(std::str::from_utf8(&manifest_bytes).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let expected = manifest_paths(&manifest)?;
    if archive.len() != expected.len() + 1 {
        return Err("Unexpected number of ZIP entries".into());
    }
    let mut seen = BTreeSet::new();
    let mut total_declared = 0u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|e| e.to_string())?;
        let key = entry.name();
        if !safe_path(key)
            || (!expected.contains_key(key) && key != "manifest.toml")
            || !seen.insert(key.to_string())
        {
            return Err(format!("Unsafe or unexpected ZIP entry: {key}"));
        }
        if entry.is_dir() {
            return Err("Directory entries not supported".into());
        }
        if let Some(mode) = entry.unix_mode() {
            if mode & 0o170000 == 0o120000 {
                return Err("ZIP symlink rejected".into());
            }
        }
        if entry.size() > MAX_FILE_BYTES && key != "manifest.toml" {
            return Err("ZIP file exceeds size limit".into());
        }
        total_declared = total_declared
            .checked_add(entry.size())
            .ok_or("ZIP size overflow")?;
        if total_declared > MAX_TOTAL_BYTES + MAX_MANIFEST_BYTES {
            return Err("ZIP too large".into());
        }
    }
    let backups = config::data_dir()?.join(&manifest.profile);
    fs::create_dir_all(&backups).map_err(|e| e.to_string())?;
    let id = format!(
        "{}-import-{}",
        Utc::now().format("%Y%m%dT%H%M%S%.fZ"),
        std::process::id()
    );
    let stage = backups.join(format!(".incoming-{id}"));
    let destination = backups.join(&id);
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
        fs::write(stage.join("manifest.toml"), &manifest_bytes).map_err(|e| e.to_string())?;
        let mut total = 0u64;
        for (key, expected_hash) in expected {
            let mut item = archive.by_name(&key).map_err(|e| e.to_string())?;
            let target = stage.join(&key);
            fs::create_dir_all(target.parent().ok_or("Missing parent")?)
                .map_err(|e| e.to_string())?;
            let mut output = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|e| e.to_string())?;
            let (hash, size) = digest(&mut item, &mut output)?;
            total += size;
            if total > MAX_TOTAL_BYTES {
                return Err("Archive exceeds total limit".into());
            }
            if let Some(expected) = expected_hash {
                if expected != hash {
                    return Err(format!("SHA-256 mismatch: {key}"));
                }
            }
        }
        fs::rename(&stage, &destination).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    result?;
    output::success(&format!("Imported profile: {}", manifest.profile));
    output::info(&format!("Backup ID: {id}"));
    output::warn(
        "Import does not restore files. Check your profile paths and use restore --dry-run first.",
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsafe_paths_rejected() {
        for p in [
            "../escape",
            "/absolute",
            "C:/windows",
            "data\\escape",
            "data/../escape",
            "",
        ] {
            assert!(!safe_path(p), "{p}");
        }
        assert!(safe_path("data/settings.json"));
    }
    #[test]
    fn manifest_alias_restricted() {
        let m = Manifest {
            profile: "dev".into(),
            created: "now".into(),
            files: BTreeMap::new(),
            entries: vec![Entry {
                alias: "x".into(),
                relative: "x/../bad".into(),
                sha256: "a".repeat(64),
            }],
        };
        assert!(manifest_paths(&m).is_err());
    }
}
