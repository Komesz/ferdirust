use std::fs;
use std::path::{Path, PathBuf};

use crate::service::config::ServiceConfig;

/// Root of all CEF profile data (CefSettings.root_cache_path).
pub fn cef_root_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/share"))
        .join("ferdirust")
        .join("cef_cache")
}

/// Per-service storage partition. Must be a child of root_cache_path so CEF
/// accepts it as a request context cache_path. Prefixed to avoid colliding
/// with Chromium-managed dirs like "Default".
pub fn partition_dir(service_id: &str) -> PathBuf {
    cef_root_dir().join(format!("svc-{service_id}"))
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// One-time migration of login state from the old shared "Default" profile
/// into a service's own partition. Copies cookies and Local Storage (sqlite/
/// leveldb — not splittable per-origin, so every partition gets a full copy)
/// plus the service's own IndexedDB origins. Deliberately skips "Service
/// Worker": workers re-register on load, and a stale SW cache is the usual
/// cause of a service booting to a white page.
pub fn migrate_from_shared_profile(svc: &ServiceConfig, dest: &Path) {
    if dest.exists() {
        return;
    }
    let shared = cef_root_dir().join("Default");
    if !shared.exists() {
        return;
    }
    if let Err(e) = fs::create_dir_all(dest) {
        eprintln!("[partition] {}: failed to create {}: {e}", svc.id, dest.display());
        return;
    }
    eprintln!(
        "[partition] {}: migrating login data from shared profile into {}",
        svc.id,
        dest.display()
    );

    for rel in [
        "Cookies",
        "Cookies-journal",
        "Network/Cookies",
        "Network/Cookies-journal",
    ] {
        let from = shared.join(rel);
        if from.is_file() {
            let to = dest.join(rel);
            if let Some(parent) = to.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Err(e) = fs::copy(&from, &to) {
                eprintln!("[partition] {}: failed to copy {rel}: {e}", svc.id);
            }
        }
    }

    let local_storage = shared.join("Local Storage");
    if local_storage.is_dir() {
        if let Err(e) = copy_dir_recursive(&local_storage, &dest.join("Local Storage")) {
            eprintln!("[partition] {}: failed to copy Local Storage: {e}", svc.id);
        }
    }

    let indexed_db = shared.join("IndexedDB");
    if let Ok(entries) = fs::read_dir(&indexed_db) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !svc.allowed_origins.iter().any(|origin| name.contains(origin.as_str())) {
                continue;
            }
            let target = dest.join("IndexedDB").join(&name);
            let result = if entry.path().is_dir() {
                copy_dir_recursive(&entry.path(), &target)
            } else {
                target
                    .parent()
                    .map(fs::create_dir_all)
                    .transpose()
                    .and_then(|_| fs::copy(entry.path(), &target))
                    .map(|_| ())
            };
            if let Err(e) = result {
                eprintln!("[partition] {}: failed to copy IndexedDB/{name}: {e}", svc.id);
            }
        }
    }
}

const WIPE_MARKER: &str = ".wipe-on-restart";

/// Flag a partition for deletion at next app start. The profile's files are
/// held open by CEF for the app's whole lifetime, so the actual delete
/// happens in sweep_marked_partitions() before CEF initializes.
pub fn mark_for_wipe(dir: &Path) {
    let _ = fs::create_dir_all(dir);
    if let Err(e) = fs::write(dir.join(WIPE_MARKER), b"") {
        eprintln!("[partition] failed to mark {} for wipe: {e}", dir.display());
    }
}

/// Delete every partition marked for wipe. Must run in the browser process
/// BEFORE CEF initializes, while no profile files are open. The emptied dir
/// is recreated so migrate_from_shared_profile doesn't resurrect old logins
/// from the legacy shared profile.
pub fn sweep_marked_partitions() {
    let root = cef_root_dir();
    let Ok(entries) = fs::read_dir(&root) else { return };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() || !dir.join(WIPE_MARKER).is_file() {
            continue;
        }
        eprintln!("[partition] wiping {}", dir.display());
        if let Err(e) = fs::remove_dir_all(&dir) {
            eprintln!("[partition] failed to wipe {}: {e}", dir.display());
            // At least drop the marker so we don't retry forever
            let _ = fs::remove_file(dir.join(WIPE_MARKER));
        }
        let _ = fs::create_dir_all(&dir);
    }
}
