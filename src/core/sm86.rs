//! Experimental Frame Generation for RTX 30-series GPUs via `dlssg_for_sm86`.
//!
//! The runtime is never bundled with Studio. The latest release zip is fetched
//! from the upstream repository, verified against the digest published by the
//! GitHub API, and cached under the components directory. Deploying copies
//! `version.dll`, `dlssg_sm86.ini` and the `dlssg_sm86\` directory next to the
//! game executable; every touched file is tracked in the journal so
//! `Restore originals` undoes the install.

use crate::core::addons::is_newer_version;
use crate::core::downloader;
use crate::core::journal::{self, ActiveManifest, ManifestGame};
use crate::core::routes::common::{track_and_copy, track_and_write};
use crate::core::scan::GameEntry;
use crate::core::state::{load_state, save_state};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub const UPSTREAM_REPO: &str = "sdli1995/dlssg_for_sm86";
pub const PROXY_DLL: &str = "version.dll";
pub const INI_NAME: &str = "dlssg_sm86.ini";
pub const BUNDLE_DIR: &str = "dlssg_sm86";
const CACHE_DIR: &str = "dlssg-sm86";
const ASSET_PREFIX: &str = "dlssg-release-";
const ASSET_SUFFIX: &str = "-x64.zip";
/// Proxy load order from the upstream INI guidance: `version.dll` first, then
/// the `alternatives\` tool proxies, render-path proxies last (never both).
/// One of these is auto-picked per game: the first slot that is free or
/// already owned by Studio. A slot occupied by a foreign file is skipped.
pub const PROXY_PRIORITY: [&str; 6] = [
    "version.dll",
    "winmm.dll",
    "dbghelp.dll",
    "dinput8.dll",
    "dxgi.dll",
    "d3d12.dll",
];
/// Keys this module manages in the deployed INI. Everything else keeps the
/// upstream release defaults.
const MANAGED_INI_KEYS: [(&str, &str, &str); 5] = [
    ("General", "Enabled", "1"),
    ("FrameGeneration", "Engine", "DLSS"),
    ("Logging", "Level", "1"),
    ("Logging", "Directory", "dlssg_sm86\\logs"),
    ("Runtime", "Mode", "Bundled"),
];
/// Keep this many cached runtime versions: the current one and one previous,
/// so a manually deployed older runtime is still recognized as ours.
const KEEP_CACHE_VERSIONS: usize = 2;
/// Minimum interval between automatic update checks (tolerates restarts).
const CHECK_INTERVAL_SECS: u64 = 20 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatestRelease {
    pub tag: String,
    pub asset_name: String,
    pub url: String,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DailyOutcome {
    /// A check ran during this call (as opposed to being rate-limit skipped).
    pub checked: bool,
    pub latest: Option<String>,
    /// Newly cached runtime version, if a background download happened.
    pub downloaded: Option<String>,
    pub error: Option<String>,
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn cache_root() -> PathBuf {
    crate::core::addons::cache::get_components_root().join(CACHE_DIR)
}

pub fn payload_dir(tag: &str) -> PathBuf {
    cache_root().join(tag)
}

/// Newest cached runtime version. Directory names are the upstream tags
/// (e.g. `0.4.1`); anything that is not a version-shaped directory is ignored.
pub fn cached_version() -> Option<String> {
    let root = cache_root();
    let mut best: Option<String> = None;
    let _ = fs::read_dir(&root).map(|entries| {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let clean = name.trim_start_matches('v').to_string();
            if !clean.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                continue;
            }
            let is_newer = best
                .as_ref()
                .map(|b| is_newer_version(b, &clean))
                .unwrap_or(true);
            if is_newer {
                best = Some(clean);
            }
        }
    });
    best
}

/// A cached payload is usable when the files the game actually loads exist.
pub fn is_cached_valid(tag: &str) -> bool {
    let dir = payload_dir(tag);
    dir.join(PROXY_DLL).is_file()
        && dir.join(INI_NAME).is_file()
        && dir
            .join(BUNDLE_DIR)
            .join("runtime")
            .join("nvngx_dlssg.dll")
            .is_file()
}

/// Query the upstream GitHub API for the newest release and its zip asset.
pub async fn latest_release() -> Result<LatestRelease, String> {
    let client = reqwest::Client::builder()
        .user_agent(concat!(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) DLSS-Studio/",
            env!("CARGO_PKG_VERSION")
        ))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let url = format!("https://api.github.com/repos/{UPSTREAM_REPO}/releases/latest");
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("GitHub API returned HTTP {}", resp.status()));
    }
    let rel: Value = resp.json().await.map_err(|e| e.to_string())?;
    let tag = rel
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim_start_matches('v')
        .to_string();
    if tag.is_empty() {
        return Err("Release has no tag".to_string());
    }
    let assets = rel
        .get("assets")
        .and_then(|v| v.as_array())
        .ok_or("Release has no assets")?;
    for asset in assets {
        let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.starts_with(ASSET_PREFIX) && name.ends_with(ASSET_SUFFIX) {
            let dl = asset
                .get("browser_download_url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if dl.is_empty() {
                break;
            }
            // GitHub publishes the asset digest as "sha256:<hex>".
            let sha256 = asset
                .get("digest")
                .and_then(|v| v.as_str())
                .and_then(|d| d.strip_prefix("sha256:"))
                .map(|s| s.to_string());
            return Ok(LatestRelease {
                tag,
                asset_name: name.to_string(),
                url: dl,
                sha256,
            });
        }
    }
    Err("No runtime zip asset found in the latest release".to_string())
}

/// Download the given release into the cache and validate it. Idempotent: an
/// already-cached version is kept as is.
pub async fn download_release(rel: &LatestRelease, log: &mut Vec<String>) -> Result<String, String> {
    if is_cached_valid(&rel.tag) {
        return Ok(rel.tag.clone());
    }
    let zip_path = std::env::temp_dir().join(format!(
        "{CACHE_DIR}-{}-{}.zip",
        rel.tag,
        std::process::id()
    ));
    let expected = rel.sha256.clone().unwrap_or_default();
    downloader::download_file_with_progress(
        &rel.url,
        &zip_path,
        &expected,
        "DLSSG for SM86",
        CACHE_DIR,
        |_| {},
    )
    .await?;
    if !expected.is_empty() {
        let actual = downloader::compute_sha256(&zip_path)
            .map_err(|e| format!("Cannot hash the downloaded runtime: {e}"))?;
        if !actual.eq_ignore_ascii_case(&expected) {
            let _ = fs::remove_file(&zip_path);
            return Err("SM86 download failed the SHA-256 integrity check".to_string());
        }
    }
    let out = payload_dir(&rel.tag);
    let _ = fs::remove_dir_all(&out);
    let file = fs::File::open(&zip_path).map_err(|e| e.to_string())?;
    let result = downloader::extract::extract_zip(file, &out);
    let _ = fs::remove_file(&zip_path);
    result?;
    if !is_cached_valid(&rel.tag) {
        let _ = fs::remove_dir_all(&out);
        return Err("Extracted runtime is incomplete".to_string());
    }
    cleanup_old_cache(&rel.tag);
    log.push(format!("[SM86] Cached upstream runtime {0}", rel.tag));
    Ok(rel.tag.clone())
}

/// Convenience wrapper used by the UI: resolve latest, then download it.
pub async fn download_latest(log: &mut Vec<String>) -> Result<String, String> {
    let rel = latest_release().await?;
    download_release(&rel, log).await
}

/// Newest first by version, mtime only as the tiebreaker.
fn version_order(a: &str, b: &str) -> std::cmp::Ordering {
    if is_newer_version(a, b) {
        std::cmp::Ordering::Less
    } else if is_newer_version(b, a) {
        std::cmp::Ordering::Greater
    } else {
        std::cmp::Ordering::Equal
    }
}

fn cleanup_old_cache(keep: &str) {
    let Ok(entries) = fs::read_dir(cache_root()) else {
        return;
    };
    let mut versions: Vec<(String, std::time::SystemTime)> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| {
            (
                e.file_name().to_string_lossy().to_string(),
                e.metadata().and_then(|m| m.modified()).unwrap_or_else(|_| std::time::SystemTime::UNIX_EPOCH),
            )
        })
        .collect();
    versions.sort_by(|a, b| version_order(&a.0, &b.0).then_with(|| b.1.cmp(&a.1)));
    for (name, _) in versions.iter().skip(KEEP_CACHE_VERSIONS) {
        if name != keep {
            let _ = fs::remove_dir_all(payload_dir(name));
        }
    }
}

/// Hourly wake-up; the 20h persisted gate makes it a daily check.
pub async fn daily_check(force: bool) -> DailyOutcome {
    let mut state = load_state();
    if state.sm86_update_mode == "off" && !force {
        return DailyOutcome::default();
    }
    let now = now_secs();
    if !force && state.sm86_last_check.is_some_and(|t| now.saturating_sub(t) < CHECK_INTERVAL_SECS) {
        return DailyOutcome {
            latest: state.sm86_upstream_version.clone(),
            ..DailyOutcome::default()
        };
    }
    match latest_release().await {
        Ok(rel) => {
            state.sm86_last_check = Some(now);
            state.sm86_upstream_version = Some(rel.tag.clone());
            let mut out = DailyOutcome {
                checked: true,
                latest: Some(rel.tag.clone()),
                ..DailyOutcome::default()
            };
            if state.sm86_update_mode == "auto" {
                let need = cached_version()
                    .map(|c| is_newer_version(&c, &rel.tag))
                    .unwrap_or(true);
                if need {
                    let mut log = Vec::new();
                    match download_release(&rel, &mut log).await {
                        Ok(v) => out.downloaded = Some(v),
                        Err(e) => out.error = Some(e),
                    }
                }
            }
            let _ = save_state(&state);
            out
        }
        Err(e) => DailyOutcome {
            error: Some(e),
            ..DailyOutcome::default()
        },
    }
}

/// Files are deployed next to the game executable (upstream SM86 install rule).
pub fn deploy_dir(game: &GameEntry) -> PathBuf {
    if game.exe_path.as_os_str().is_empty() {
        game.dir.clone()
    } else {
        game.exe_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| game.dir.clone())
    }
}

pub fn installed_version(game: &GameEntry) -> Option<String> {
    journal::read_manifest(&game.dir)
        .and_then(|m| m.sm86_version)
}

/// Multiplier written into a game's deployed INI ("4X" = MaxGeneratedFrames 3).
pub fn installed_multiplier(mod_root: &Path) -> Option<u32> {
    let text = fs::read_to_string(mod_root.join(INI_NAME)).ok()?;
    let generated = crate::core::ini::get_ini(&text, "FrameGeneration", "MaxGeneratedFrames")?
        .parse::<u32>()
        .ok()?;
    (1..=5).contains(&generated).then(|| generated + 1)
}

/// Rewrite the INI text with this module's managed keys and the multiplier
/// ceiling. Managed keys are stripped first (duplicate keys confuse the
/// runtime's INI reader), then re-set, so an old 6x value can never linger.
pub fn configure_ini(base: &str, multiplier: u32) -> Result<String, String> {
    if !(2..=6).contains(&multiplier) {
        return Err("SM86 multiplier must be 2x, 3x, 4x, 5x or 6x".to_string());
    }
    let mut section = String::new();
    let newline = if base.contains("\r\n") { "\r\n" } else { "\n" };
    let mut text: String = base
        .lines()
        .filter(|line| {
            let trimmed = line.trim().trim_start_matches('\u{feff}');
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                section = trimmed[1..trimmed.len() - 1].trim().to_string();
            }
            if trimmed.starts_with(';') || trimmed.starts_with('#') {
                return true;
            }
            let Some((key, _)) = trimmed.split_once('=') else {
                return true;
            };
            let managed = MANAGED_INI_KEYS
                .iter()
                .any(|(s, k, _)| section.eq_ignore_ascii_case(s) && key.trim().eq_ignore_ascii_case(k));
            let managed_max_generated = section.eq_ignore_ascii_case("FrameGeneration")
                && key.trim().eq_ignore_ascii_case("MaxGeneratedFrames");
            !(managed || managed_max_generated)
        })
        .collect::<Vec<_>>()
        .join(newline);
    for (section, key, value) in MANAGED_INI_KEYS {
        text = crate::core::ini::set_ini(&text, section, key, value);
    }
    Ok(crate::core::ini::set_ini(
        &text,
        "FrameGeneration",
        "MaxGeneratedFrames",
        &(multiplier - 1).to_string(),
    ))
}

/// One issue from the runtime's own log: the raw line plus a human hint key
/// (empty = no known hint, the raw line is shown as-is).
pub struct Sm86Issue {
    pub raw: String,
    pub hint_key: &'static str,
}

/// Read the runtime log (`dlssg_sm86\logs\dlssg_sm86.log`, last 8 KiB) and
/// return its last few lines with translated hints for the ones we recognize.
/// The runtime logs at Level=1 (errors) by default, so the tail is what
/// matters. Lines carry no stable delimiter we could rely on, so hints come
/// from keyword matches only.
pub fn runtime_issues(deploy_dir: &Path, max_lines: usize) -> Vec<Sm86Issue> {
    let path = deploy_dir
        .join(BUNDLE_DIR)
        .join("logs")
        .join("dlssg_sm86.log");
    let Ok(meta) = fs::metadata(&path) else {
        return Vec::new();
    };
    let len = meta.len().min(8192) as usize;
    let Ok(mut file) = fs::File::open(&path) else {
        return Vec::new();
    };
    use std::io::{Read, Seek, SeekFrom};
    let _ = file.seek(SeekFrom::End(-(len as i64)));
    let mut buf = vec![0u8; len];
    let _ = file.read_exact(&mut buf);
    let text = String::from_utf8_lossy(&buf);
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..]
        .iter()
        .map(|line| {
            let lower = line.to_lowercase();
            let hint_key = if lower.contains("configuration_invalid") {
                "sm86_hint_cfg_invalid"
            } else if lower.contains("max_generated_frames_unsupported") {
                "sm86_hint_frames_unsupported"
            } else if lower.contains("driver") || lower.contains("nvapi") {
                "sm86_hint_driver"
            } else if lower.contains("runtime") || lower.contains("not found") || lower.contains("missing") {
                "sm86_hint_runtime"
            } else {
                ""
            };
            Sm86Issue {
                raw: line.trim().to_string(),
                hint_key,
            }
        })
        .collect()
}

fn owns_rel(manifest: &ActiveManifest, rel: &str) -> bool {
    manifest.added.iter().any(|a| a == rel)
        || manifest
            .replaced
            .iter()
            .any(|r| r.rel == rel)
}

/// A pre-existing proxy file counts as ours when its bytes match any proxy
/// file in the given payload or any cached runtime version (covers manual
/// installs done outside Studio).
fn is_known_proxy_bytes(path: &Path, payload: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if !PROXY_PRIORITY.iter().any(|n| n.eq_ignore_ascii_case(name)) {
        return false;
    }
    let Ok(actual) = downloader::compute_sha256(path) else {
        return false;
    };
    let mut roots = vec![payload.to_path_buf()];
    if let Ok(entries) = fs::read_dir(cache_root()) {
        roots.extend(entries.flatten().map(|e| e.path()));
    }
    for root in roots {
        for rel in [PathBuf::from(name), PathBuf::from("alternatives").join(name)] {
            let candidate = root.join(&rel);
            if candidate.is_file() {
                if let Ok(expected) = downloader::compute_sha256(&candidate) {
                    if expected.eq_ignore_ascii_case(&actual) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Auto-pick the proxy slot for a game: the first candidate that is free, or
/// already ours (bytes match the payload or a cached runtime, or the journal
/// owns the slot). Foreign files make their slot unavailable; render-path
/// proxies are naturally last in the priority list.
pub fn pick_proxy(mod_root: &Path, game_dir: &Path, payload: &Path) -> Result<&'static str, String> {
    let manifest = journal::read_manifest(game_dir);
    for name in PROXY_PRIORITY {
        let path = mod_root.join(name);
        if !path.is_file() {
            return Ok(name);
        }
        if is_known_proxy_bytes(&path, payload) {
            return Ok(name);
        }
        if let (Some(m), Some(rel)) = (&manifest, rel_path(game_dir, &path)) {
            if owns_rel(m, &rel) {
                return Ok(name);
            }
        }
    }
    Err(format!(
        "Every proxy slot is taken by other mods ({}). Remove one of them or restore the originals first.",
        PROXY_PRIORITY.join(", ")
    ))
}

fn new_manifest(game: &GameEntry, backup_prefix: &str, exe_rel: &str) -> ActiveManifest {
    ActiveManifest {
        version: 1,
        date: journal::now_timestamp_str(),
        route: "sm86".to_string(),
        game: Some(ManifestGame {
            dir: Some(game.dir.to_string_lossy().to_string()),
            exe: Some(exe_rel.to_string()),
            api: Some(game.api.clone()),
            bitness: Some(game.bitness),
            api_label: Some(game.api.clone()),
        }),
        game_exe: Some(exe_rel.to_string()),
        backup_prefix: Some(backup_prefix.to_string()),
        ..Default::default()
    }
}

/// Deploy the cached runtime into a game. Fails closed: refuses to run while
/// the game is open and refuses foreign files in every proxy slot.
pub fn install(game: &GameEntry, multiplier: u32, log: &mut Vec<String>) -> Result<(), String> {
    let version = cached_version()
        .ok_or_else(|| "Download the SM86 runtime first".to_string())?;
    if !is_cached_valid(&version) {
        return Err(format!(
            "Cached SM86 runtime {version} is incomplete; download it again"
        ));
    }
    install_from(&payload_dir(&version), &version, multiplier, game, log)
}

/// Testable install core: payload dir, version and multiplier are passed
/// explicitly. Layers into an existing route manifest when one is present
/// (e.g. OptiScaler + SM86 side by side) and creates a standalone `sm86`
/// manifest otherwise.
pub fn install_from(
    payload: &Path,
    version: &str,
    multiplier: u32,
    game: &GameEntry,
    log: &mut Vec<String>,
) -> Result<(), String> {
    let exe_opt = if game.exe_path.as_os_str().is_empty() {
        None
    } else {
        Some(game.exe_path.as_path())
    };
    crate::core::install_guards::assert_game_closed(&game.dir, exe_opt)
        .map_err(|e| format!("Close the game before installing: {e}"))?;

    let mod_root = deploy_dir(game);
    let picked = pick_proxy(&mod_root, &game.dir, payload)?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let prefix = format!("originals/{ts}");
    let bdir = journal::backup_dir(&game.dir).join(&prefix);
    let existing = journal::read_manifest(&game.dir);
    let mut manifest = existing.unwrap_or_else(|| new_manifest(game, &prefix, &rel_exe(game)));

    // Update path: drop files we already own (including a previous proxy under
    // a different slot name) so track_and_copy records the new bytes as plain
    // additions instead of backing up our own previous files.
    for name in PROXY_PRIORITY {
        let dest = mod_root.join(name);
        if name != picked {
            if let Some(rel) = rel_path(&game.dir, &dest) {
                if dest.is_file() && owns_rel(&manifest, &rel) {
                    let _ = fs::remove_file(&dest);
                    manifest.added.retain(|a| *a != rel);
                    manifest.replaced.retain(|r| r.rel != rel);
                }
            }
            continue;
        }
        if dest.is_file() {
            if let Some(rel) = rel_path(&game.dir, &dest) {
                if owns_rel(&manifest, &rel) {
                    let _ = fs::remove_file(&dest);
                }
            }
        }
    }
    {
        let ini_dest = mod_root.join(INI_NAME);
        if let Some(rel) = rel_path(&game.dir, &ini_dest) {
            if ini_dest.is_file() && owns_rel(&manifest, &rel) {
                let _ = fs::remove_file(&ini_dest);
            }
        }
        // Stale runtime files from the previous version: remove the ones the
        // journal tracks under the bundle directory, keep anything untracked
        // (e.g. the runtime's own logs).
        if let Some(bundle_rel) = rel_path(&game.dir, &mod_root.join(BUNDLE_DIR)) {
            let prefix = format!("{bundle_rel}\\");
            let mut stale: Vec<String> = manifest
                .added
                .iter()
                .filter(|r| r.starts_with(&prefix))
                .cloned()
                .collect();
            stale.extend(
                manifest
                    .replaced
                    .iter()
                    .filter(|r| r.rel.starts_with(&prefix))
                    .map(|r| r.rel.clone()),
            );
            for rel in &stale {
                let _ = fs::remove_file(game.dir.join(rel));
            }
            manifest.added.retain(|a| !stale.contains(a));
            manifest.replaced.retain(|r| !stale.contains(&r.rel));
            if !manifest.added_dirs.contains(&bundle_rel) {
                manifest.added_dirs.push(bundle_rel);
            }
        }
    }

    let proxy_dest = mod_root.join(picked);
    track_and_copy(&mut manifest, &game.dir, &bdir, &payload.join(PROXY_DLL), &proxy_dest, "sm86", log)
        .map_err(|e| format!("Failed to deploy {picked}: {e}"))?;
    let ini_text = configure_ini(
        &fs::read_to_string(payload.join(INI_NAME))
            .map_err(|e| format!("Cached runtime INI is unreadable: {e}"))?,
        multiplier,
    )?;
    track_and_write(&mut manifest, &game.dir, &bdir, &mod_root.join(INI_NAME), &ini_text, "sm86", log)
        .map_err(|e| format!("Failed to deploy {}: {e}", INI_NAME))?;
    copy_bundle(payload, &game.dir, &mod_root, &bdir, &mut manifest, log)?;

    manifest.sm86_version = Some(version.to_string());
    journal::save_manifest(&game.dir, &manifest)
        .map_err(|e| format!("Failed to save the journal: {e}"))?;
    log.push(format!(
        "[SM86] Deployed runtime {version} ({multiplier}x, proxy {picked}) into {}",
        mod_root.display()
    ));
    Ok(())
}

/// Remove only this module's files from a game, leaving any other deployed
/// route (OptiScaler, ReShade, ...) intact. Updates the manifest in place.
pub fn uninstall(game: &GameEntry, log: &mut Vec<String>) -> Result<(), String> {
    let exe_opt = exe_opt(game);
    crate::core::install_guards::assert_game_closed(&game.dir, exe_opt)
        .map_err(|e| format!("Close the game before removing SM86: {e}"))?;

    let mut manifest = journal::read_manifest(&game.dir)
        .ok_or_else(|| "No journal manifest for this game".to_string())?;

    let is_sm86_rel = |rel: &str| -> bool {
        let name = rel.rsplit('\\').next().unwrap_or(rel);
        let name = name.rsplit('/').next().unwrap_or(name);
        name == INI_NAME
            || PROXY_PRIORITY.iter().any(|n| *n == name)
            || rel.contains(BUNDLE_DIR)
    };

    for rel in manifest.added.clone() {
        if is_sm86_rel(&rel) {
            let target = journal::resolve_target_path(&game.dir, &rel);
            if target.exists() {
                let _ = fs::remove_file(&target);
            }
            log.push(format!("[SM86] Removed {}", rel));
        }
    }
    manifest.added.retain(|r| !is_sm86_rel(r));

    let bundle_path = deploy_dir(game).join(BUNDLE_DIR);
    if bundle_path.exists() {
        let _ = fs::remove_dir_all(&bundle_path);
    }
    manifest.added_dirs.retain(|d| !d.contains(BUNDLE_DIR));

    let bdir = journal::backup_dir(&game.dir);
    let prefix = manifest.backup_prefix.clone();
    let to_restore: Vec<_> = manifest.replaced.iter().filter(|r| is_sm86_rel(&r.rel)).cloned().collect();
    for item in &to_restore {
        let mut backup_file = if let Some(ref p) = prefix {
            bdir.join(p).join(&item.rel)
        } else {
            bdir.join(&item.rel)
        };
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
        let target = journal::resolve_target_path(&game.dir, &item.rel);
        if backup_file.exists() {
            if let Some(parent) = target.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::copy(&backup_file, &target);
        } else {
            let _ = fs::remove_file(&target);
        }
    }
    manifest.replaced.retain(|r| !is_sm86_rel(&r.rel));

    manifest.sm86_version = None;
    journal::save_manifest(&game.dir, &manifest)
        .map_err(|e| format!("Failed to save the journal: {e}"))?;
    log.push("[SM86] Removed runtime".to_string());
    Ok(())
}

fn exe_opt(game: &GameEntry) -> Option<&Path> {
    if game.exe_path.as_os_str().is_empty() {
        None
    } else {
        Some(game.exe_path.as_path())
    }
}

fn rel_exe(game: &GameEntry) -> String {
    game.exe_path
        .strip_prefix(&game.dir)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn rel_path(game_dir: &Path, p: &Path) -> Option<String> {
    p.strip_prefix(game_dir)
        .ok()
        .map(|r| r.to_string_lossy().to_string())
}

fn copy_bundle(
    payload: &Path,
    game_dir: &Path,
    mod_root: &Path,
    bdir: &Path,
    manifest: &mut ActiveManifest,
    log: &mut Vec<String>,
) -> Result<(), String> {
    let src_root = payload.join(BUNDLE_DIR);
    let walk = |dir: &Path,
                manifest: &mut ActiveManifest,
                log: &mut Vec<String>|
     -> Result<(), String> {
        let mut stack = vec![dir.to_path_buf()];
        while let Some(current) = stack.pop() {
            let entries = fs::read_dir(&current)
                .map_err(|e| format!("Cannot read {}: {e}", current.display()))?;
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    let dest = mod_root.join(path.strip_prefix(payload).unwrap_or(&path));
                    track_and_copy(manifest, game_dir, bdir, &path, &dest, "sm86", log)
                        .map_err(|e| format!("Failed to deploy {}: {e}", path.display()))?;
                }
            }
        }
        Ok(())
    };
    walk(&src_root, manifest, log)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_payload(root: &Path, marker: &str) -> PathBuf {
        let dir = root.join("payload");
        fs::create_dir_all(dir.join(format!("{BUNDLE_DIR}/runtime"))).unwrap();
        fs::create_dir_all(dir.join(format!("{BUNDLE_DIR}/sr"))).unwrap();
        fs::write(dir.join(PROXY_DLL), format!("proxy-{marker}")).unwrap();
        fs::write(dir.join(INI_NAME), format!("[DLSSG]\nmarker={marker}\n")).unwrap();
        fs::write(
            dir.join(format!("{BUNDLE_DIR}/runtime/nvngx_dlssg.dll")),
            format!("runtime-{marker}"),
        )
        .unwrap();
        fs::write(
            dir.join(format!("{BUNDLE_DIR}/sr/sr_kernels.dll")),
            format!("sr-{marker}"),
        )
        .unwrap();
        dir
    }

    fn temp_game(root: &Path) -> GameEntry {
        let dir = root.join("game");
        let bin = dir.join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("game.exe"), b"game").unwrap();
        GameEntry {
            name: "Test Game".into(),
            dir: dir.clone(),
            exe_path: bin.join("game.exe"),
            api: "d3d12".into(),
            ..Default::default()
        }
    }

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("sm86-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn install_update_remove_roundtrip() {
        let root = temp_root("roundtrip");
        let payload = fake_payload(&root, "v1");
        let game = temp_game(&root);
        let mut log = Vec::new();

        install_from(&payload, "0.4.1", 4, &game, &mut log).unwrap();
        let deploy = game.exe_path.parent().unwrap();
        assert_eq!(fs::read(deploy.join(PROXY_DLL)).unwrap(), b"proxy-v1");
        assert_eq!(
            installed_multiplier(deploy),
            Some(4),
            "multiplier must be written into the deployed INI"
        );
        assert!(deploy
            .join(BUNDLE_DIR)
            .join("runtime")
            .join("nvngx_dlssg.dll")
            .is_file());
        assert!(deploy.join(BUNDLE_DIR).join("sr").join("sr_kernels.dll").is_file());

        let manifest = journal::read_manifest(&game.dir).unwrap();
        assert_eq!(manifest.route, "sm86");
        assert_eq!(manifest.sm86_version.as_deref(), Some("0.4.1"));
        assert!(manifest.added.contains(&format!("bin\\{PROXY_DLL}")));
        assert!(manifest.added_dirs.iter().any(|d| d.starts_with("bin\\dlssg_sm86")));

        // Update with different bytes and a lower multiplier replaces everything we own.
        let payload2 = fake_payload(&root.join("p2"), "v2");
        install_from(&payload2, "0.4.2", 2, &game, &mut log).unwrap();
        assert_eq!(fs::read(deploy.join(PROXY_DLL)).unwrap(), b"proxy-v2");
        assert_eq!(installed_multiplier(deploy), Some(2));
        let manifest = journal::read_manifest(&game.dir).unwrap();
        assert_eq!(manifest.sm86_version.as_deref(), Some("0.4.2"));
        assert!(manifest.replaced.is_empty(), "owned files must not be journaled as replaced originals");

        journal::restore_game(&game.dir).unwrap();
        assert!(!deploy.join(PROXY_DLL).exists());
        assert!(!deploy.join(INI_NAME).exists());
        assert!(!deploy.join(BUNDLE_DIR).exists());
        assert!(journal::read_manifest(&game.dir).is_none());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn picks_next_free_proxy() {
        let root = temp_root("nextfree");
        let payload = fake_payload(&root, "v1");
        let game = temp_game(&root);
        let mut log = Vec::new();
        let deploy = game.exe_path.parent().unwrap();
        fs::write(deploy.join(PROXY_DLL), b"some other mod").unwrap();

        install_from(&payload, "0.4.1", 4, &game, &mut log).unwrap();
        assert_eq!(
            fs::read(deploy.join(PROXY_DLL)).unwrap(),
            b"some other mod",
            "foreign file must be left untouched"
        );
        assert_eq!(
            fs::read(deploy.join("winmm.dll")).unwrap(),
            b"proxy-v1",
            "install falls back to the next free slot"
        );
        assert!(journal::read_manifest(&game.dir).is_some());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn errors_when_all_proxy_slots_foreign() {
        let root = temp_root("allslots");
        let payload = fake_payload(&root, "v1");
        let game = temp_game(&root);
        let mut log = Vec::new();
        let deploy = game.exe_path.parent().unwrap();
        for name in PROXY_PRIORITY {
            fs::write(deploy.join(name), b"foreign").unwrap();
        }

        let err = install_from(&payload, "0.4.1", 4, &game, &mut log).unwrap_err();
        assert!(err.contains("Every proxy slot is taken"), "{err}");
        assert!(journal::read_manifest(&game.dir).is_none());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn merges_into_existing_route_manifest() {
        let root = temp_root("merge");
        let payload = fake_payload(&root, "v1");
        let game = temp_game(&root);
        let mut log = Vec::new();
        let mut manifest = ActiveManifest {
            route: "optiscaler".into(),
            ..Default::default()
        };
        manifest.added.push("bin\\dxgi.dll".into());
        journal::save_manifest(&game.dir, &manifest).unwrap();

        install_from(&payload, "0.4.1", 4, &game, &mut log).unwrap();
        let manifest = journal::read_manifest(&game.dir).unwrap();
        assert_eq!(manifest.route, "optiscaler", "layered install keeps the owning route");
        assert_eq!(manifest.sm86_version.as_deref(), Some("0.4.1"));
        assert!(manifest.added.contains(&format!("bin\\{PROXY_DLL}")));

        journal::restore_game(&game.dir).unwrap();
        assert!(!game.exe_path.parent().unwrap().join(PROXY_DLL).exists());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn multiplier_validation_and_ini_rewrite() {
        assert!(configure_ini("", 7).is_err());
        assert!(configure_ini("", 1).is_err());
        let ini = configure_ini(
            "[FrameGeneration]\nMaxGeneratedFrames=5\nmaxgeneratedframes=99\n[Unrelated]\nKey=value\n",
            3,
        )
        .unwrap();
        assert_eq!(
            crate::core::ini::get_ini(&ini, "FrameGeneration", "MaxGeneratedFrames"),
            Some("2".to_string()),
            "duplicate managed keys must be collapsed to the single new value"
        );
        assert_eq!(
            crate::core::ini::get_ini(&ini, "Unrelated", "Key"),
            Some("value".to_string()),
            "unrelated settings must survive"
        );
    }
}
