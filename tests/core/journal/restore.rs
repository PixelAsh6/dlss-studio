use dlss_studio::core::journal::restore::*;
use dlss_studio::core::journal::manifest::{backup_dir, ActiveManifest, ManifestItem};
use crate::common::TempDir;
use std::fs;

#[test]
fn test_has_backup_available() {
    let temp = TempDir::new("restore_has_backup");
    let bdir = temp.join("_DLSS5_Backup");
    fs::create_dir_all(&bdir).unwrap();

    assert!(!has_backup_available(temp.path()));

    let manifest = ActiveManifest {
        route: "optiscaler".to_string(),
        ..Default::default()
    };
    let bytes = serde_json::to_vec(&manifest).unwrap();
    fs::write(bdir.join("manifest.json"), &bytes).unwrap();
    assert!(has_backup_available(temp.path()));
}

#[test]
fn test_running_game_guard_prevents_restore() {
    let procs = dlss_studio::core::install_guards::get_running_processes();
    if procs.is_empty() {
        return;
    }
    let running_proc_name = &procs[0].name;

    let temp = TempDir::new("guard_restore");
    let running_dummy_exe = temp.join(running_proc_name);
    fs::write(&running_dummy_exe, b"MZ").unwrap();

    let manifest = ActiveManifest {
        game_exe: Some(running_proc_name.clone()),
        ..Default::default()
    };
    let bdir = backup_dir(temp.path());
    fs::create_dir_all(&bdir).unwrap();
    fs::write(bdir.join("manifest.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();

    let restore_res = restore_game(temp.path());
    assert!(restore_res.is_err(), "Restore must be blocked when game process is active");
    let rest_err = restore_res.unwrap_err().to_string();
    assert!(rest_err.contains("Close the game"), "Error must tell user to close the game first: {}", rest_err);
}

#[test]
fn test_restore_game_purges_corrupted_backup_and_historically_added_files() {
    let temp = TempDir::new("restore_clean");

    // Vanilla game file
    fs::write(temp.join("sl.interposer.dll"), b"VANILLA_INTERPOSER").unwrap();
    // Leftover injected file sitting in game dir
    fs::write(temp.join("optiscaler.ini"), b"INJECTED_OPTISCALER_INI").unwrap();
    // OptiScaler files
    fs::write(temp.join("dxgi.dll"), b"OPTISCALER_DXGI").unwrap();

    let bdir = temp.join("_DLSS5_Backup");
    let prefix = "originals/test1";
    let orig_dir = bdir.join(prefix);
    fs::create_dir_all(&orig_dir).unwrap();

    // Legitimate vanilla backup
    fs::write(orig_dir.join("sl.interposer.dll"), b"VANILLA_INTERPOSER").unwrap();
    // CORRUPTED BACKUP: optiscaler.ini was mistakenly put in backup by prior dirty state
    fs::write(orig_dir.join("optiscaler.ini"), b"INJECTED_OPTISCALER_INI").unwrap();

    // Done manifest from an even earlier run that had added optiscaler.ini
    let done_manifest = ActiveManifest {
        added: vec!["optiscaler.ini".to_string()],
        ..Default::default()
    };
    fs::write(bdir.join("manifest.json.done-100"), serde_json::to_vec(&done_manifest).unwrap()).unwrap();

    // Current active manifest mistakenly has optiscaler.ini in replaced
    let active_manifest = ActiveManifest {
        backup_prefix: Some(prefix.to_string()),
        replaced: vec![
            ManifestItem { rel: "sl.interposer.dll".to_string(), old_hash: None, kind: None },
            ManifestItem { rel: "optiscaler.ini".to_string(), old_hash: None, kind: None },
        ],
        added: vec!["dxgi.dll".to_string()],
        ..Default::default()
    };
    fs::write(bdir.join("manifest.json"), serde_json::to_vec(&active_manifest).unwrap()).unwrap();

    // Execute restore
    let res = restore_game(temp.path()).expect("restore must succeed");
    assert!(res);

    // Assert pristine state:
    // 1. sl.interposer.dll was restored
    assert_eq!(fs::read(temp.join("sl.interposer.dll")).unwrap(), b"VANILLA_INTERPOSER");
    // 2. dxgi.dll was deleted
    assert!(!temp.join("dxgi.dll").exists());
    // 3. optiscaler.ini was PURGED (not restored from corrupted backup)
    assert!(!temp.join("optiscaler.ini").exists(), "Corrupted backup optiscaler.ini must be purged, not restored!");
}

#[test]
fn test_restore_game_preserves_and_restores_vanilla_sl_pcl_dll() {
    let temp = TempDir::new("restore_vanilla_pcl");

    // Game originally shipped with CDPR vanilla Streamline files
    let bdir = temp.join("_DLSS5_Backup");
    let prefix = "originals/witcher3";
    let orig_dir = bdir.join(prefix);
    fs::create_dir_all(&orig_dir).unwrap();

    // DLSS Studio safely backed up genuine game files
    fs::write(orig_dir.join("sl.interposer.dll"), b"VANILLA_INTERPOSER").unwrap();
    fs::write(orig_dir.join("sl.pcl.dll"), b"VANILLA_CDPR_PCL").unwrap();

    // Active manifest tracks that sl.interposer.dll and sl.pcl.dll were replaced, and dxgi.dll was added
    let active_manifest = ActiveManifest {
        backup_prefix: Some(prefix.to_string()),
        replaced: vec![
            ManifestItem { rel: "sl.interposer.dll".to_string(), old_hash: None, kind: None },
            ManifestItem { rel: "sl.pcl.dll".to_string(), old_hash: None, kind: None },
        ],
        added: vec!["dxgi.dll".to_string()],
        ..Default::default()
    };
    fs::write(bdir.join("manifest.json"), serde_json::to_vec(&active_manifest).unwrap()).unwrap();

    // Current state in game directory (active mod deployed)
    fs::write(temp.join("sl.interposer.dll"), b"UPGRADED_STREAMLINE").unwrap();
    fs::write(temp.join("sl.pcl.dll"), b"TOUCHED_PCL").unwrap();
    fs::write(temp.join("dxgi.dll"), b"OPTISCALER_HOOK").unwrap();

    // Execute restore
    let res = restore_game(temp.path()).expect("restore must succeed");
    assert!(res);

    // CRITICAL: Vanilla sl.pcl.dll and sl.interposer.dll MUST be fully restored to original vanilla bytes!
    assert_eq!(fs::read(temp.join("sl.pcl.dll")).unwrap(), b"VANILLA_CDPR_PCL", "Vanilla sl.pcl.dll must be restored byte-for-byte!");
    assert_eq!(fs::read(temp.join("sl.interposer.dll")).unwrap(), b"VANILLA_INTERPOSER", "Vanilla sl.interposer.dll must be restored byte-for-byte!");
    // Added mod files must be deleted
    assert!(!temp.join("dxgi.dll").exists(), "Added mod hook must be deleted on restore");
}

#[test]
fn test_restore_game_deletes_streamline_when_game_had_no_streamline() {
    let temp = TempDir::new("restore_added_sl");

    let bdir = temp.join("_DLSS5_Backup");
    fs::create_dir_all(&bdir).unwrap();

    // Game did NOT have Streamline. Streamline files were added by DLSS Studio
    let active_manifest = ActiveManifest {
        added: vec![
            "dxgi.dll".to_string(),
            "sl.interposer.dll".to_string(),
            "sl.common.dll".to_string(),
            "sl.dlss_g.dll".to_string(),
        ],
        ..Default::default()
    };
    fs::write(bdir.join("manifest.json"), serde_json::to_vec(&active_manifest).unwrap()).unwrap();

    // Files currently exist in game directory
    fs::write(temp.join("dxgi.dll"), b"HOOK").unwrap();
    fs::write(temp.join("sl.interposer.dll"), b"ADDED_SL").unwrap();
    fs::write(temp.join("sl.common.dll"), b"ADDED_SL").unwrap();
    fs::write(temp.join("sl.dlss_g.dll"), b"ADDED_SL").unwrap();

    // Execute restore
    let res = restore_game(temp.path()).expect("restore must succeed");
    assert!(res);

    // CRITICAL: All added Streamline files must be deleted since the game had no Streamline originally!
    assert!(!temp.join("dxgi.dll").exists());
    assert!(!temp.join("sl.interposer.dll").exists(), "Added Streamline must be deleted on restore");
    assert!(!temp.join("sl.common.dll").exists(), "Added Streamline must be deleted on restore");
    assert!(!temp.join("sl.dlss_g.dll").exists(), "Added Streamline must be deleted on restore");
}
