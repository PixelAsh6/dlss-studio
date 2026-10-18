use std::fs;
use std::path::Path;
use super::clean::{clean_untracked_mods, clean_untracked_mods_with_exe, is_corrupted_or_mod_backup};
use super::manifest::{backup_dir, read_manifest, resolve_target_path};

pub fn has_backup_available(game_dir: &Path) -> bool {
    let bdir = backup_dir(game_dir);
    bdir.join("manifest.json").exists()
}

pub fn restore_game(game_dir: &Path) -> std::io::Result<bool> {
    crate::core::logger::info("restore", &format!("Initiating restore for game: {}", game_dir.display()));
    let manifest = match read_manifest(game_dir) {
        Some(m) => m,
        None => {
            // Fallback: clean untracked mods if no manifest is found
            let cleaned = clean_untracked_mods(game_dir)?;
            let bdir = backup_dir(game_dir);
            if bdir.exists() {
                let originals = bdir.join("originals");
                if !originals.exists() || fs::read_dir(&originals).map(|mut d| d.next().is_none()).unwrap_or(true) {
                    let _ = fs::remove_dir_all(&bdir);
                }
            }
            crate::core::logger::info("restore", &format!("No active manifest found for {}; cleaned untracked mods: {:?}", game_dir.display(), cleaned));
            return Ok(!cleaned.is_empty());
        }
    };

    let exe_opt = manifest.game_exe.as_ref()
        .or_else(|| manifest.game.as_ref().and_then(|g| g.exe.as_ref()))
        .map(|e| game_dir.join(e));

    if let Err(e) = crate::core::install_guards::assert_game_closed(game_dir, exe_opt.as_deref()) {
        crate::core::logger::warn("restore", &format!("Cannot restore while game is running: {}", e));
        return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, e));
    }

    let bdir = backup_dir(game_dir);

    // ========================================================
    // STEP 1: Delete all added mod files, directories & untracked mods
    // ========================================================
    for rel in &manifest.added {
        let target_file = resolve_target_path(game_dir, rel);
        if target_file.exists() {
            let _ = fs::remove_file(&target_file);
            crate::core::logger::debug("restore", &format!("Removed mod file: {}", target_file.display()));
        }
    }

    for rel in manifest.added_dirs.iter().rev() {
        let target_dir = resolve_target_path(game_dir, rel);
        if target_dir.exists() {
            let _ = fs::remove_dir_all(&target_dir);
            crate::core::logger::debug("restore", &format!("Removed mod directory: {}", target_dir.display()));
        }
    }

    // Also purge known leftover injected mod files and unregister vulkan layer
    let _ = clean_untracked_mods_with_exe(game_dir, exe_opt.as_deref());
    let _ = crate::core::vulkan_layer::unregister_vulkan_layer(game_dir);

    // ========================================================
    // STEP 2: Restore all original files from backup
    // ========================================================
    for item in &manifest.replaced {
        let mut backup_file = if let Some(ref p) = manifest.backup_prefix {
            bdir.join(p).join(&item.rel)
        } else {
            bdir.join(&item.rel)
        };

        // Fallback: search in originals/ or other prefix folders if not at primary path
        if !backup_file.exists() {
            let direct = bdir.join("originals").join(&item.rel);
            if direct.is_file() {
                backup_file = direct;
            } else if let Ok(entries) = fs::read_dir(bdir.join("originals")) {
                for entry in entries.flatten() {
                    let candidate = entry.path().join(&item.rel);
                    if candidate.is_file() {
                        backup_file = candidate;
                        break;
                    }
                }
            }
        }

        let target_file = resolve_target_path(game_dir, &item.rel);
        if backup_file.exists() {
            // Safety check: if the backed-up file was actually a proxy hook or mod file
            // that was mistakenly copied to backup during a prior dirty install, NEVER restore it!
            if is_corrupted_or_mod_backup(&backup_file) {
                crate::core::logger::warn("restore", &format!("Skipping corrupted backup file (is proxy hook or mod): {}", backup_file.display()));
                if target_file.exists() {
                    let _ = fs::remove_file(&target_file);
                }
                continue;
            }

            if let Some(parent) = target_file.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&backup_file, &target_file)?;
            crate::core::logger::debug("restore", &format!("Restored original file: {}", target_file.display()));
        }
    }

    // ========================================================
    // STEP 3: Remove _DLSS5_Backup entirely (pristine game folder)
    // ========================================================
    if bdir.exists() {
        let _ = fs::remove_dir_all(&bdir);
        crate::core::logger::info("restore", &format!("Removed backup directory: {}", bdir.display()));
    }

    crate::core::logger::info("restore", &format!("Restore successfully completed for {}", game_dir.display()));

    Ok(true)
}
