use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestItem {
    pub rel: String,
    #[serde(default)]
    pub old_hash: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestGame {
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(default)]
    pub exe: Option<String>,
    #[serde(default)]
    pub api: Option<String>,
    #[serde(default)]
    pub bitness: Option<u32>,
    #[serde(default)]
    pub api_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveManifest {
    #[serde(default = "default_manifest_version")]
    pub version: u32,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub route: String,
    #[serde(default)]
    pub game: Option<ManifestGame>,
    #[serde(default)]
    pub game_exe: Option<String>,
    #[serde(default)]
    pub backup_prefix: Option<String>,
    #[serde(default)]
    pub replaced: Vec<ManifestItem>,
    #[serde(default)]
    pub added: Vec<String>,
    #[serde(default)]
    pub added_dirs: Vec<String>,
    #[serde(default)]
    pub mfg_unlock: Option<bool>,
    #[serde(default)]
    pub mfg_multiplier: Option<u32>,
    #[serde(default)]
    pub nr_style_enabled: Option<bool>,
    #[serde(default)]
    pub nr_style: Option<usize>,
    #[serde(default)]
    pub opti_presr: Option<bool>,
    #[serde(default)]
    pub opti_passes: Option<u32>,
}

fn default_manifest_version() -> u32 {
    1
}

impl Default for ActiveManifest {
    fn default() -> Self {
        Self {
            version: 1,
            date: format!("{:?}", std::time::SystemTime::now()),
            route: "optiscaler".to_string(),
            game: None,
            game_exe: Some(String::new()),
            backup_prefix: None,
            replaced: Vec::new(),
            added: Vec::new(),
            added_dirs: Vec::new(),
            mfg_unlock: None,
            mfg_multiplier: None,
            nr_style_enabled: None,
            nr_style: None,
            opti_presr: None,
            opti_passes: None,
        }
    }
}

pub fn strip_verbatim(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

pub fn backup_dir(game_dir: &Path) -> PathBuf {
    // 1. Direct subfolder
    let direct = game_dir.join("_DLSS5_Backup");
    if direct.exists() {
        return direct;
    }

    // 2. Ancestor of game_dir if game_dir is deeply nested e.g. in "Content/NewMoon/Binaries/WinGDK"
    for ancestor in game_dir.ancestors().skip(1).take(5) {
        let p_backup = ancestor.join("_DLSS5_Backup");
        if p_backup.exists() {
            return p_backup;
        }
    }

    // 3. Canonicalized path (resolving NTFS Junctions / symlinks like Xbox WindowsApps -> Games\...\Content)
    if let Ok(canon) = fs::canonicalize(game_dir) {
        let norm = strip_verbatim(&canon);
        let c_direct = norm.join("_DLSS5_Backup");
        if c_direct.exists() {
            return c_direct;
        }
        for ancestor in norm.ancestors().skip(1).take(5) {
            let p_backup = ancestor.join("_DLSS5_Backup");
            if p_backup.exists() {
                return p_backup;
            }
        }
    }

    direct
}

pub fn read_manifest(game_dir: &Path) -> Option<ActiveManifest> {
    let bdir = backup_dir(game_dir);
    let manifest_file = bdir.join("manifest.json");
    if let Ok(bytes) = fs::read(&manifest_file) {
        if let Ok(m) = serde_json::from_slice::<ActiveManifest>(&bytes) {
            return Some(m);
        }
    }
    None
}

pub fn save_manifest(game_dir: &Path, manifest: &ActiveManifest) -> std::io::Result<()> {
    let replaced_rels: Vec<&str> = manifest.replaced.iter().map(|r| r.rel.as_str()).collect();
    crate::core::logger::info(
        "journal",
        &format!(
            "Saving active manifest for {}: route={}, replaced={} {:?}, added={} {:?}",
            game_dir.display(),
            manifest.route,
            manifest.replaced.len(),
            replaced_rels,
            manifest.added.len(),
            manifest.added
        ),
    );
    let bdir = backup_dir(game_dir);
    fs::create_dir_all(&bdir)?;
    let bytes = serde_json::to_vec_pretty(manifest)?;
    fs::write(bdir.join("manifest.json"), bytes)?;
    Ok(())
}

pub fn resolve_target_path(game_dir: &Path, rel: &str) -> PathBuf {
    let direct = game_dir.join(rel);
    if direct.exists() {
        return direct;
    }
    // Check stripped Content prefix if game_dir is already Content or a junction to Content
    if let Ok(stripped) = Path::new(rel).strip_prefix("Content") {
        let sub = game_dir.join(stripped);
        if sub.exists() {
            return sub;
        }
    }
    if let Ok(stripped) = Path::new(rel).strip_prefix("content") {
        let sub = game_dir.join(stripped);
        if sub.exists() {
            return sub;
        }
    }
    // Check game_dir.join("Content").join(rel)
    let c_sub = game_dir.join("Content").join(rel);
    if c_sub.exists() {
        return c_sub;
    }

    if let Ok(canon) = fs::canonicalize(game_dir) {
        let norm = strip_verbatim(&canon);
        let c_direct = norm.join(rel);
        if c_direct.exists() {
            return c_direct;
        }
        if let Some(parent) = norm.parent() {
            let p_direct = parent.join(rel);
            if p_direct.exists() {
                return p_direct;
            }
        }
    }

    if let Ok(stripped) = Path::new(rel).strip_prefix("Content") {
        game_dir.join(stripped)
    } else {
        direct
    }
}
