use dlss_studio::core::journal::manifest::*;
use crate::common::TempDir;
use std::fs;

#[test]
fn test_has_backup_available_active_manifest_only() {
    let temp = TempDir::new("journal_manifest");
    let bdir = temp.join("_DLSS5_Backup");
    fs::create_dir_all(&bdir).unwrap();

    assert!(read_manifest(temp.path()).is_none());

    // Legacy done manifest should NOT trigger active manifest
    let manifest = ActiveManifest {
        route: "optiscaler".to_string(),
        ..Default::default()
    };
    let bytes = serde_json::to_vec(&manifest).unwrap();
    fs::write(bdir.join("manifest.json.done-123456789"), &bytes).unwrap();

    assert!(read_manifest(temp.path()).is_none(), "read_manifest must ignore legacy done manifests");

    // When active manifest.json is present, return Some
    fs::write(bdir.join("manifest.json"), &bytes).unwrap();
    let active = read_manifest(temp.path());
    assert!(active.is_some());
    assert_eq!(active.unwrap().route, "optiscaler");
}

#[test]
fn test_manifest_serialization() {
    let temp = TempDir::new("manifest_ser");

    // Manifest path
    let m_path = backup_dir(temp.path()).join("manifest.json");
    assert!(m_path.ends_with("manifest.json"));

    // Read manifest on non-existent file
    assert!(read_manifest(temp.path()).is_none());

    // Read manifest with invalid JSON
    fs::create_dir_all(backup_dir(temp.path())).unwrap();
    fs::write(&m_path, b"invalid-json").unwrap();
    assert!(read_manifest(temp.path()).is_none());

    // Save active manifest
    let manifest = ActiveManifest {
        route: "native".to_string(),
        added: vec!["renodx-dlss.addon64".to_string()],
        ..Default::default()
    };
    save_manifest(temp.path(), &manifest).unwrap();

    let read_back = read_manifest(temp.path()).expect("save_manifest must persist readable manifest");
    assert_eq!(read_back.route, "native");
    assert_eq!(read_back.added, vec!["renodx-dlss.addon64".to_string()]);
}
