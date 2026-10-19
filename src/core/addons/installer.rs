use std::fs;
use std::path::{Path, PathBuf};
use crate::core::downloader::{
    download_file_with_progress, download_file_with_sha256, DownloadProgress, DownloadStage,
};
use super::catalog::*;
use super::cache::*;
use super::version::*;
use super::{AddonId, ALL_REFERENCED_ADDONS, DgVoodooComponents, FeederComponents};

/// Helper to execute tar silently on Windows without popping cmd/conhost consoles
fn silent_tar_command() -> std::process::Command {
    let mut cmd = std::process::Command::new("tar");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Helper function to extract a ZIP archive safely using std::fs and zip crate
pub fn extract_zip(file: fs::File, out_dir: &Path) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Failed to read zip: {}", e))?;
    for i in 0..archive.len() {
        let mut f = archive.by_index(i).map_err(|e| format!("Zip entry error: {}", e))?;
        let outpath = match f.enclosed_name() {
            Some(path) => out_dir.join(path),
            None => continue,
        };
        if f.is_dir() {
            fs::create_dir_all(&outpath).map_err(|e| format!("Create dir error: {}", e))?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p).map_err(|e| format!("Create parent dir error: {}", e))?;
                }
            }
            let mut outfile = fs::File::create(&outpath).map_err(|e| format!("File create error: {}", e))?;
            std::io::copy(&mut f, &mut outfile).map_err(|e| format!("Extraction copy error: {}", e))?;
        }
    }
    Ok(())
}

/// Extracts ReShade32.dll and ReShade64.dll from the ReShade Add-on Setup archive
pub fn extract_reshade_from_setup(setup_path: &Path, comp_root: &Path) -> Result<(), String> {
    let _ = fs::create_dir_all(comp_root);
    let file = fs::File::open(setup_path).map_err(|e| format!("Failed to open ReShade setup: {}", e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Failed to open ReShade archive: {}", e))?;

    let reshade32_dest = comp_root.join("ReShade32.dll");
    let reshade64_dest = comp_root.join("ReShade64.dll");

    let reshade32_json = comp_root.join("ReShade32.json");
    let reshade64_json = comp_root.join("ReShade64.json");

    for i in 0..archive.len() {
        if let Ok(mut entry) = archive.by_index(i) {
            let name = entry.name().to_string();
            if name.ends_with("ReShade32.dll") {
                if let Ok(mut out) = fs::File::create(&reshade32_dest) {
                    let _ = std::io::copy(&mut entry, &mut out);
                }
            } else if name.ends_with("ReShade64.dll") {
                if let Ok(mut out) = fs::File::create(&reshade64_dest) {
                    let _ = std::io::copy(&mut entry, &mut out);
                }
            } else if name.ends_with("ReShade32.json") {
                if let Ok(mut out) = fs::File::create(&reshade32_json) {
                    let _ = std::io::copy(&mut entry, &mut out);
                }
            } else if name.ends_with("ReShade64.json") {
                if let Ok(mut out) = fs::File::create(&reshade64_json) {
                    let _ = std::io::copy(&mut entry, &mut out);
                }
            }
        }
    }

    if reshade64_dest.is_file() {
        Ok(())
    } else {
        Err("ReShade64.dll could not be extracted from setup".to_string())
    }
}

/// Selectively extracts 64-bit Streamline runtime DLLs from the official NVIDIA Streamline SDK zip.
/// Only extracts files in `bin/x64/` (ignoring `development/`, docs, headers, and debug symbols).
pub fn extract_streamline_from_sdk(file: fs::File, out_dir: &Path) -> Result<(), String> {
    let _ = fs::create_dir_all(out_dir);
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Failed to read Streamline SDK zip: {}", e))?;

    let has_bin_x64 = (0..archive.len()).any(|i| {
        if let Ok(entry) = archive.by_index(i) {
            let n = entry.name().replace('\\', "/");
            n.contains("bin/x64/") && !n.contains("/development/") && n.to_lowercase().ends_with(".dll")
        } else {
            false
        }
    });

    let mut extracted_count = 0;
    for i in 0..archive.len() {
        let mut f = archive.by_index(i).map_err(|e| format!("Zip entry error: {}", e))?;
        if f.is_dir() {
            continue;
        }
        let name = f.name().to_string();
        let normalized = name.replace('\\', "/");

        let matches = if has_bin_x64 {
            normalized.contains("bin/x64/") && !normalized.contains("/development/") && !normalized.contains("/debug/")
        } else {
            true
        };

        if matches {
            if let Some(file_name) = Path::new(&normalized).file_name().and_then(|s| s.to_str()) {
                let lower = file_name.to_lowercase();
                if !lower.ends_with(".dll") {
                    continue;
                }
                // Only take Streamline / DLSS DLLs: sl.*, nvngx_dlss*.dll
                if lower.starts_with("sl.") || lower.starts_with("nvngx_dlss") {
                    let outpath = out_dir.join(file_name);
                    let mut outfile = fs::File::create(&outpath).map_err(|e| format!("Failed to create {}: {}", outpath.display(), e))?;
                    std::io::copy(&mut f, &mut outfile).map_err(|e| format!("Extraction error for {}: {}", file_name, e))?;
                    extracted_count += 1;
                }
            }
        }
    }

    if extracted_count == 0 {
        return Err("No Streamline runtime DLLs found in archive".to_string());
    }
    Ok(())
}

/// Cleans stale directories, older versions, and temporary archives for a specific add-on.
pub fn clean_stale_addon_cache(comp_root: &Path, id: AddonId, keep_target: Option<&Path>) {
    let canonical_keep = keep_target.and_then(|p| p.canonicalize().ok()).or_else(|| keep_target.map(|p| p.to_path_buf()));

    if let Ok(entries) = fs::read_dir(comp_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_dir = path.is_dir();
            let file_name = entry.file_name().to_string_lossy().to_string();
            let lower_name = file_name.to_lowercase();

            // Never delete the keep target
            if let Some(ref keep) = canonical_keep {
                if path == *keep || path.canonicalize().ok().as_ref() == Some(keep) {
                    continue;
                }
            }

            let should_delete = match id {
                AddonId::RenoDxMfgUnlock => {
                    (is_dir && lower_name.starts_with("mfg-unlock-"))
                        || (!is_dir && lower_name.starts_with("mfg-unlock-") && lower_name.ends_with(".zip"))
                        || (!is_dir && lower_name.starts_with("renodx-mfgunlock") && lower_name.ends_with(".zip"))
                }
                AddonId::Streamline => {
                    if is_dir && lower_name.starts_with("streamline-") {
                        let target_streamline = comp_root.join("streamline");
                        let sub_dlssnr = path.join("streamline").join("nvngx_dlssnr.dll");
                        let direct_dlssnr = path.join("nvngx_dlssnr.dll");
                        if (sub_dlssnr.is_file() || direct_dlssnr.is_file()) && !target_streamline.join("nvngx_dlssnr.dll").is_file() {
                            let _ = fs::create_dir_all(&target_streamline);
                            let src = if sub_dlssnr.is_file() { sub_dlssnr } else { direct_dlssnr };
                            let _ = fs::copy(&src, target_streamline.join("nvngx_dlssnr.dll"));
                        }
                    }
                    (is_dir && (lower_name.starts_with("streamline-") || (lower_name == "streamline" && keep_target.is_some() && canonical_keep.as_ref() != Some(&path))))
                        || (!is_dir && lower_name.starts_with("streamline") && lower_name.ends_with(".zip"))
                }
                AddonId::Dlss5Feeder => {
                    (is_dir && (lower_name.starts_with("dlss5-feeder-") || lower_name == "dlss5-feeder"))
                        || (!is_dir && lower_name.starts_with("dlss5-feeder") && lower_name.ends_with(".zip"))
                }
                AddonId::OptiScaler => {
                    (is_dir && lower_name.starts_with("optiscaler-"))
                        || (!is_dir && lower_name.starts_with("optiscaler") && lower_name.ends_with(".zip"))
                }
                AddonId::Rtx40Mfg => {
                    (is_dir && (lower_name.starts_with("rtxmfg-") || lower_name.starts_with("rtx40mfg-")))
                        || (!is_dir && (lower_name.starts_with("rtxmfg") || lower_name.starts_with("rtx40mfg")) && lower_name.ends_with(".zip"))
                }
                AddonId::DgVoodoo => {
                    !is_dir && lower_name.starts_with("dgvoodoo2") && lower_name.ends_with(".zip")
                }
                AddonId::ReShade => {
                    !is_dir && lower_name.starts_with("reshade_setup") && lower_name.ends_with(".exe")
                }
                AddonId::RenoDxDlss5 => {
                    (is_dir && (lower_name == "renodx-dlss5" || (lower_name == "renodx-dlss" && keep_target.is_some() && canonical_keep.as_ref() != Some(&path))))
                        || (!is_dir && (lower_name.starts_with("renodx-dlss5") || lower_name.starts_with("renodx-dlss")) && lower_name.ends_with(".zip"))
                }
                AddonId::DlssMipFix => {
                    is_dir && lower_name == "dlss5-d3d12-fix"
                }
            };

            if should_delete {
                if is_dir {
                    let _ = fs::remove_dir_all(&path);
                } else {
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }
}

/// Sweeps all add-on components on application startup to purge any orphaned directories or archives.
pub fn clean_all_stale_addon_caches(comp_root: &Path) {
    // 1. RenoDX MFG Unlock: preserve highest version or mfg-unlock-1.0
    let mfg_active = {
        let mut candidates = Vec::new();
        if let Ok(entries) = fs::read_dir(comp_root) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if e.path().is_dir() && name.starts_with("mfg-unlock-") && e.path().join("renodx-mfgunlock.addon64").is_file() {
                    candidates.push(e.path());
                }
            }
        }
        candidates.sort_by(|a, b| b.cmp(a));
        candidates.into_iter().next().unwrap_or_else(|| comp_root.join("mfg-unlock-1.0"))
    };
    clean_stale_addon_cache(comp_root, AddonId::RenoDxMfgUnlock, Some(&mfg_active));

    // 2. Streamline: preserve components/streamline
    let streamline_target = comp_root.join("streamline");
    let legacy_dlssnr = comp_root.join("streamline-2.14.1").join("streamline").join("nvngx_dlssnr.dll");
    if legacy_dlssnr.is_file() && !streamline_target.join("nvngx_dlssnr.dll").is_file() {
        let _ = fs::create_dir_all(&streamline_target);
        let _ = fs::copy(&legacy_dlssnr, streamline_target.join("nvngx_dlssnr.dll"));
    }
    clean_stale_addon_cache(comp_root, AddonId::Streamline, Some(&streamline_target));

    // 3. DLSS 5 Feeder: preserve highest version
    let feeder_active = {
        let mut candidates = Vec::new();
        if let Ok(entries) = fs::read_dir(comp_root) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if e.path().is_dir() && name.starts_with("DLSS5-Feeder-") && e.path().join("dlss5-feed.addon64").is_file() {
                    candidates.push(e.path());
                }
            }
        }
        candidates.sort_by(|a, b| b.cmp(a));
        candidates.into_iter().next()
    };
    clean_stale_addon_cache(comp_root, AddonId::Dlss5Feeder, feeder_active.as_deref());

    // 4. OptiScaler: preserve highest version
    let opti_active = {
        let mut candidates = Vec::new();
        if let Ok(entries) = fs::read_dir(comp_root) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if e.path().is_dir() && name.starts_with("OptiScaler-") && e.path().join("OptiScaler.dll").is_file() {
                    candidates.push(e.path());
                }
            }
        }
        candidates.sort_by(|a, b| b.cmp(a));
        candidates.into_iter().next()
    };
    clean_stale_addon_cache(comp_root, AddonId::OptiScaler, opti_active.as_deref());

    // 5. RTX40MFG: preserve mfg-standalone
    let rtxmfg_active = comp_root.join("mfg-standalone");
    clean_stale_addon_cache(comp_root, AddonId::Rtx40Mfg, Some(&rtxmfg_active));

    // 6. dgVoodoo2: preserve dgvoodoo
    clean_stale_addon_cache(comp_root, AddonId::DgVoodoo, Some(&comp_root.join("dgvoodoo")));

    // 7. ReShade
    clean_stale_addon_cache(comp_root, AddonId::ReShade, None);

    // 8. RenoDx Engine: preserve renodx-dlss
    clean_stale_addon_cache(comp_root, AddonId::RenoDxDlss5, Some(&comp_root.join("renodx-dlss")));

    // 9. D3D12 Mip Fix: preserve dlss-mip-fix
    clean_stale_addon_cache(comp_root, AddonId::DlssMipFix, Some(&comp_root.join("dlss-mip-fix")));
}

/// Downloads, installs, or updates a single core add-on package
pub async fn install_or_update_addon<F>(
    id: AddonId,
    force: bool,
    mut progress_fn: F,
) -> Result<(), String>
where
    F: FnMut(DownloadProgress),
{
    let comp_root = get_components_root();
    let _ = fs::create_dir_all(&comp_root);

    let info = check_addon_update(id).await;
    let target_tag = info.latest_version.clone().unwrap_or_else(|| get_local_addon_version(id).unwrap_or_else(|| "latest".to_string()));
    
    crate::core::state::log_message(&format!("@{{log_addon_updating|{}|{}}}", id.name(), target_tag));

    match id {
        AddonId::RenoDxDlss5 => {
            let renodx_dir = comp_root.join("renodx-dlss");
            let target_dest = renodx_dir.join("renodx-dlss.addon64");
            if !force && target_dest.is_file() {
                return Ok(());
            }
            let _ = fs::create_dir_all(&renodx_dir);

            // Check local build / repository candidate files first before downloading
            let mut local_candidates = vec![
                PathBuf::from("external_assets/renodx-dlss.addon64"),
                PathBuf::from("dist/renodx-dlss.addon64"),
                PathBuf::from("target/release/renodx-dlss.addon64"),
                PathBuf::from("target/debug/renodx-dlss.addon64"),
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("external_assets").join("renodx-dlss.addon64"),
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dist").join("renodx-dlss.addon64"),
            ];
            if let Ok(exe_p) = std::env::current_exe() {
                if let Some(exe_dir) = exe_p.parent() {
                    local_candidates.push(exe_dir.join("dist").join("renodx-dlss.addon64"));
                    local_candidates.push(exe_dir.join("renodx-dlss.addon64"));
                    local_candidates.push(exe_dir.join("external_assets").join("renodx-dlss.addon64"));
                    if let Some(parent) = exe_dir.parent() {
                        local_candidates.push(parent.join("external_assets").join("renodx-dlss.addon64"));
                        local_candidates.push(parent.join("dist").join("renodx-dlss.addon64"));
                    }
                }
            }
            let mut copied = false;
            for cand in &local_candidates {
                if cand.is_file() {
                    if fs::copy(cand, &target_dest).is_ok() {
                        copied = true;
                        break;
                    }
                }
            }

            if !copied {
                let dl_url = info.download_url.unwrap_or_else(|| RENODX_DLSS_URL.to_string());
                download_file_with_progress(
                    &dl_url,
                    &target_dest,
                    RENODX_DLSS_SHA256,
                    &format!("RenoDX DLSS ({})", target_tag),
                    "renodx",
                    &mut progress_fn,
                ).await?;
            }
            crate::core::state::log_message(&format!("@{{log_addon_download_success|{}|{}}}", id.name(), target_tag));
            let tag_to_record = target_tag.clone();
            clean_stale_addon_cache(&comp_root, AddonId::RenoDxDlss5, Some(&renodx_dir));
            let _ = record_addon_version(AddonId::RenoDxDlss5, &tag_to_record);
        }
        AddonId::DlssMipFix => {
            if !force && is_dlss5_d3d12_fix_cached() {
                return Ok(());
            }
            let fix_dir = comp_root.join("dlss-mip-fix");
            let _ = fs::create_dir_all(&fix_dir);
            let target_dest = fix_dir.join("dlss-mip-fix.addon64");

            // Check local build candidates first before downloading
            let local_candidates = [
                PathBuf::from("target/release/dlss_mip_addon.dll"),
                PathBuf::from("target/release/dlss-mip-fix.addon64"),
                PathBuf::from("target/debug/dlss_mip_addon.dll"),
                PathBuf::from("target/debug/dlss-mip-fix.addon64"),
                PathBuf::from("dist/dlss-mip-fix.addon64"),
                PathBuf::from("dist/dlss_mip_addon.dll"),
            ];
            let mut copied = false;
            for cand in &local_candidates {
                if cand.is_file() {
                    if fs::copy(cand, &target_dest).is_ok() {
                        copied = true;
                        break;
                    }
                }
            }

            if !copied {
                let dl_url = info.download_url.unwrap_or_else(|| DLSS_MIP_FIX_URL.to_string());
                download_file_with_progress(
                    &dl_url,
                    &target_dest,
                    "",
                    "DLSS Studio D3D12 Mip Companion",
                    "dlss_mip_fix",
                    &mut progress_fn,
                ).await?;
            }
            clean_stale_addon_cache(&comp_root, AddonId::DlssMipFix, Some(&fix_dir));
            let _ = record_addon_version(AddonId::DlssMipFix, &target_tag);
        }
        AddonId::RenoDxMfgUnlock => {
            if !force && is_mfg_addon_cached() {
                return Ok(());
            }
            let dl_url = info.download_url.unwrap_or_else(|| MFG_UNLOCK_URL.to_string());
            let mfg_dir = comp_root.join("mfg-unlock-1.0");
            let _ = fs::create_dir_all(&mfg_dir);
            let target_dest = mfg_dir.join("renodx-mfgunlock.addon64");
            download_file_with_progress(
                &dl_url,
                &target_dest,
                "",
                &format!("RenoDX 4x MFG Unlock ({})", target_tag),
                "mfg_unlock",
                &mut progress_fn,
            ).await?;
            crate::core::state::log_message(&format!("@{{log_addon_download_success|{}|{}}}", id.name(), target_tag));
            clean_stale_addon_cache(&comp_root, AddonId::RenoDxMfgUnlock, Some(&mfg_dir));
            let _ = record_addon_version(AddonId::RenoDxMfgUnlock, &target_tag);
        }
        AddonId::Dlss5Feeder => {
            if !force && is_feeder_cached() {
                return Ok(());
            }
            let dl_url = info.download_url.unwrap_or_else(|| FEEDER_ARCHIVE_URL.to_string());
            let tag_clean = target_tag.trim_start_matches('v');
            let feeder_dir = comp_root.join(format!("DLSS5-Feeder-{}", tag_clean));
            let feeder_zip = comp_root.join(format!("DLSS5-Feeder-{}.zip", tag_clean));
            download_file_with_progress(
                &dl_url,
                &feeder_zip,
                "",
                &format!("DLSS 5 Feeder ({})", target_tag),
                "feeder",
                &mut progress_fn,
            ).await?;

            crate::core::state::log_message(&format!("@{{log_addon_download_success|{}|{}}}", id.name(), target_tag));

            if let Ok(file) = fs::File::open(&feeder_zip) {
                let feeder_dir_c = feeder_dir.clone();
                let feeder_zip_c = feeder_zip.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    let _ = extract_zip(file, &feeder_dir_c);
                    let _ = fs::remove_file(&feeder_zip_c);
                }).await;
            }

            crate::core::state::log_message(&format!("@{{log_addon_unpack_success|{}|{}}}", id.name(), target_tag));

            clean_stale_addon_cache(&comp_root, AddonId::Dlss5Feeder, Some(&feeder_dir));

            let mut dummy_log = Vec::new();
            ensure_feeder_components(&mut dummy_log).await?;
            let _ = record_addon_version(AddonId::Dlss5Feeder, &target_tag);
        }
        AddonId::Rtx40Mfg => {
            if !force && is_rtxmfg_cached() {
                return Ok(());
            }
            let (dl_url, tag) = resolve_latest_dashdogy_mfg().await;
            let mfg_dir = comp_root.join("mfg-standalone");
            let _ = fs::create_dir_all(&mfg_dir);
            let zip_path = comp_root.join(format!("RTXMFG-{}.zip", tag));

            download_file_with_progress(
                &dl_url,
                &zip_path,
                "",
                &format!("Universal RTX40MFG-Unlock ({})", tag),
                "rtxmfg",
                &mut progress_fn,
            ).await?;

            crate::core::state::log_message(&format!("@{{log_addon_download_success|{}|{}}}", id.name(), tag));

            if let Ok(file) = fs::File::open(&zip_path) {
                let mfg_dir_c = mfg_dir.clone();
                let zip_path_c = zip_path.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    let _ = extract_zip(file, &mfg_dir_c);
                    let _ = fs::remove_file(&zip_path_c);
                }).await;
            }

            crate::core::state::log_message(&format!("@{{log_addon_unpack_success|{}|{}}}", id.name(), tag));
            clean_stale_addon_cache(&comp_root, AddonId::Rtx40Mfg, Some(&mfg_dir));
            let _ = record_addon_version(AddonId::Rtx40Mfg, &tag);
        }
        AddonId::OptiScaler => {
            if !force && is_optiscaler_cached() {
                return Ok(());
            }
            let (dl_url, tag) = resolve_latest_optiscaler_mfg().await;
            let tag_clean = tag.trim_start_matches('v');
            let opti_dir = comp_root.join(format!("OptiScaler-{}-dlss5", tag_clean));
            let zip_path = comp_root.join("OptiScaler-latest.zip");

            download_file_with_progress(
                &dl_url,
                &zip_path,
                "",
                &format!("OptiScaler DLSS 5 ({})", tag),
                "optiscaler",
                &mut progress_fn,
            ).await?;

            crate::core::state::log_message(&format!("@{{log_addon_download_success|{}|{}}}", id.name(), tag));

            if let Ok(file) = fs::File::open(&zip_path) {
                let opti_dir_c = opti_dir.clone();
                let zip_path_c = zip_path.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    let _ = extract_zip(file, &opti_dir_c);
                    let _ = fs::remove_file(&zip_path_c);

                    // Auto-flatten nested subfolders (e.g. DLSS5-Optimised) if OptiScaler.dll is inside
                    let candidate_subdirs = ["DLSS5-Optimised", "OptiScaler"];
                    for sub_name in &candidate_subdirs {
                        let sub_path = opti_dir_c.join(sub_name);
                        if sub_path.is_dir() && sub_path.join("OptiScaler.dll").is_file() {
                            if let Ok(entries) = fs::read_dir(&sub_path) {
                                for entry in entries.flatten() {
                                    let src = entry.path();
                                    let dest = opti_dir_c.join(entry.file_name());
                                    let _ = fs::rename(&src, &dest);
                                }
                            }
                            let _ = fs::remove_dir_all(&sub_path);
                            break;
                        }
                    }
                }).await;
            }

            crate::core::state::log_message(&format!("@{{log_addon_unpack_success|{}|{}}}", id.name(), tag));

            clean_stale_addon_cache(&comp_root, AddonId::OptiScaler, Some(&opti_dir));
            let _ = record_addon_version(AddonId::OptiScaler, &tag);
        }
        AddonId::ReShade => {
            if !force && is_reshade_cached() {
                return Ok(());
            }
            let setup_path = comp_root.join("ReShade_Setup_6.8.0_Addon.exe");
            download_file_with_progress(
                RESHADE_SETUP_URL,
                &setup_path,
                RESHADE_SETUP_SHA256,
                "ReShade 6.8.0 (Add-on Runtime)",
                "reshade",
                &mut progress_fn,
            ).await?;

            crate::core::state::log_message(&format!("@{{log_addon_download_success|{}|{}}}", id.name(), "v6.8.0"));

            let setup_path_c = setup_path.clone();
            let comp_root_c = comp_root.clone();
            let extract_res = tokio::task::spawn_blocking(move || {
                let res = extract_reshade_from_setup(&setup_path_c, &comp_root_c);
                let _ = fs::remove_file(&setup_path_c);
                res
            }).await.map_err(|e| format!("ReShade extraction task error: {}", e))?;

            extract_res?;
            crate::core::state::log_message(&format!("@{{log_addon_unpack_success|{}|{}}}", id.name(), "v6.8.0"));
            clean_stale_addon_cache(&comp_root, AddonId::ReShade, None);
            let _ = record_addon_version(AddonId::ReShade, "v6.8.0");
        }
        AddonId::Streamline => {
            if !force && is_streamline_cached() {
                return Ok(());
            }
            let streamline_dir = comp_root.join("streamline");
            let zip_path = comp_root.join("streamline-download.zip");

            let mut local_search_dirs = vec![
                PathBuf::from("external_assets"),
                PathBuf::from("dist"),
                PathBuf::from("target/release"),
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("external_assets"),
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dist"),
            ];
            if let Ok(exe_p) = std::env::current_exe() {
                if let Some(exe_dir) = exe_p.parent() {
                    local_search_dirs.push(exe_dir.join("dist"));
                    local_search_dirs.push(exe_dir.join("external_assets"));
                    if let Some(parent) = exe_dir.parent() {
                        local_search_dirs.push(parent.join("external_assets"));
                        local_search_dirs.push(parent.join("dist"));
                    }
                }
            }

            let mut copied = false;
            // 1. If download_url was resolved to a local staged file path, copy directly
            if let Some(ref dl_url) = info.download_url {
                let local_path = PathBuf::from(dl_url);
                if local_path.is_file() {
                    if fs::copy(&local_path, &zip_path).is_ok() {
                        copied = true;
                    }
                }
            }

            // 2. Otherwise scan local search directories for streamline-*.zip
            if !copied {
                for dir in &local_search_dirs {
                    if let Ok(entries) = fs::read_dir(dir) {
                        for entry in entries.flatten() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            if name.starts_with("streamline-") && name.ends_with(".zip") {
                                if fs::copy(entry.path(), &zip_path).is_ok() {
                                    copied = true;
                                    break;
                                }
                            }
                        }
                    }
                    if copied {
                        break;
                    }
                }
            }

            // 3. Fallback to network download if no local candidate was found
            if !copied {
                let dl_url = info.download_url.ok_or_else(|| {
                    "Streamline runtime bundle not found in GitHub releases or local search paths".to_string()
                })?;
                download_file_with_progress(
                    &dl_url,
                    &zip_path,
                    "",
                    &format!("Streamline Runtime ({})", target_tag),
                    "streamline",
                    &mut progress_fn,
                ).await?;
                crate::core::state::log_message(&format!("@{{log_addon_download_success|{}|{}}}", id.name(), target_tag));
            }

            if let Ok(file) = fs::File::open(&zip_path) {
                let streamline_dir_c = streamline_dir.clone();
                let zip_path_c = zip_path.clone();
                let extract_res = tokio::task::spawn_blocking(move || {
                    let r = extract_streamline_from_sdk(file, &streamline_dir_c);
                    let _ = fs::remove_file(&zip_path_c);
                    r
                }).await.map_err(|e| format!("Streamline extraction task error: {}", e))?;
                extract_res?;
            }

            clean_stale_addon_cache(&comp_root, AddonId::Streamline, Some(&streamline_dir));

            crate::core::state::log_message(&format!("@{{log_addon_unpack_success|{}|{}}}", id.name(), target_tag));
            let _ = record_addon_version(AddonId::Streamline, &target_tag);
        }
        AddonId::DgVoodoo => {
            if !force && is_dgvoodoo_cached() {
                return Ok(());
            }
            let dl_url = info.download_url.unwrap_or_else(|| DGVOODOO_URL.to_string());
            let zip_path = comp_root.join("dgVoodoo2_87_5.zip");
            download_file_with_progress(
                &dl_url,
                &zip_path,
                "",
                &format!("dgVoodoo2 ({})", target_tag),
                "dgvoodoo",
                &mut progress_fn,
            ).await?;

            crate::core::state::log_message(&format!("@{{log_addon_download_success|{}|{}}}", id.name(), target_tag));

            let comp_root_c = comp_root.clone();
            let zip_path_c = zip_path.clone();
            let extract_res = tokio::task::spawn_blocking(move || {
                let dgvoodoo_dir = comp_root_c.join("dgvoodoo");
                let _ = fs::create_dir_all(dgvoodoo_dir.join("x86"));
                let _ = fs::create_dir_all(dgvoodoo_dir.join("x64"));

                let _ = silent_tar_command()
                    .arg("-xf")
                    .arg(&zip_path_c)
                    .arg("-C")
                    .arg(&dgvoodoo_dir)
                    .arg("dgVoodoo.conf")
                    .status();

                let _ = silent_tar_command()
                    .arg("-xf")
                    .arg(&zip_path_c)
                    .arg("-C")
                    .arg(dgvoodoo_dir.join("x86"))
                    .arg("--strip-components")
                    .arg("2")
                    .arg("MS/x86/D3D9.dll")
                    .status();

                let _ = silent_tar_command()
                    .arg("-xf")
                    .arg(&zip_path_c)
                    .arg("-C")
                    .arg(dgvoodoo_dir.join("x86"))
                    .arg("--strip-components")
                    .arg("2")
                    .arg("MS/x86/D3D8.dll")
                    .status();

                let _ = silent_tar_command()
                    .arg("-xf")
                    .arg(&zip_path_c)
                    .arg("-C")
                    .arg(dgvoodoo_dir.join("x64"))
                    .arg("--strip-components")
                    .arg("2")
                    .arg("MS/x64/D3D9.dll")
                    .status();

                let _ = fs::remove_file(&zip_path_c);
            }).await;

            if let Err(e) = extract_res {
                return Err(format!("dgVoodoo2 extraction error: {}", e));
            }
            crate::core::state::log_message(&format!("@{{log_addon_unpack_success|{}|{}}}", id.name(), target_tag));
            clean_stale_addon_cache(&comp_root, AddonId::DgVoodoo, Some(&comp_root.join("dgvoodoo")));
            let _ = record_addon_version(AddonId::DgVoodoo, &target_tag);
        }
    }

    invalidate_addon_cache();
    crate::core::state::log_message(&format!("@{{log_addon_updated|{}|{}}}", id.name(), target_tag));
    Ok(())
}

/// Helper to update a single addon
pub async fn update_single_addon<F>(id: AddonId, progress_fn: F) -> Result<(), String>
where
    F: FnMut(DownloadProgress),
{
    install_or_update_addon(id, true, progress_fn).await
}

/// Helper to update multiple addons in sequence with aggregated progress
pub async fn update_all_addons<F>(ids: &[AddonId], mut progress_fn: F) -> Result<(), String>
where
    F: FnMut(DownloadProgress),
{
    let total = ids.len() as f32;
    if total == 0.0 {
        return Ok(());
    }

    for (idx, &id) in ids.iter().enumerate() {
        let step_offset = idx as f32;
        let mut step_progress = |mut p: DownloadProgress| {
            p.percentage = ((step_offset * 100.0) + p.percentage) / total;
            progress_fn(p);
        };
        install_or_update_addon(id, true, &mut step_progress).await?;
    }

    Ok(())
}

/// Sequentially checks and ensures all mandatory components are cached on disk
pub async fn ensure_mandatory_components_cached_with_progress<F>(mut progress_fn: F) -> Result<(), String>
where
    F: FnMut(DownloadProgress),
{
    let comp_root = get_components_root();
    clean_all_stale_addon_caches(&comp_root);

    let total_steps = ALL_REFERENCED_ADDONS.len() as f32;
    let mut errors = Vec::new();

    for (idx, &id) in ALL_REFERENCED_ADDONS.iter().enumerate() {
        if is_addon_cached(id) {
            continue;
        }

        // Streamline runtime bundle is managed via the dedicated Streamline Setup Guidance Modal
        // or explicit user action, avoiding silent background multi-hundred megabyte download on fresh launch.
        if id == AddonId::Streamline {
            continue;
        }

        let step_offset = idx as f32;
        let mut step_progress = |mut p: DownloadProgress| {
            p.percentage = ((step_offset * 100.0) + p.percentage) / total_steps;
            progress_fn(p);
        };

        if let Err(e) = install_or_update_addon(id, false, &mut step_progress).await {
            crate::core::state::log_message(&format!("@{{log_download_error|{:?}|{}}}", id, e));
            errors.push(format!("{:?}: {}", id, e));
        }
    }

    let core_ready = is_mfg_addon_cached()
        && is_feeder_cached()
        && is_renodx_engine_cached()
        && is_dlss5_d3d12_fix_cached()
        && is_rtxmfg_cached()
        && is_optiscaler_cached()
        && is_reshade_cached();

    if core_ready && errors.is_empty() {
        progress_fn(DownloadProgress {
            is_downloading: false,
            component_name: String::new(),
            component_id: String::new(),
            url: String::new(),
            downloaded_bytes: 0,
            total_bytes: None,
            percentage: 100.0,
            stage: DownloadStage::Downloading,
            message: "Core components ready".to_string(),
        });
        Ok(())
    } else {
        let err_msg = format!("Failed to download some mandatory components: {}", errors.join(", "));
        progress_fn(DownloadProgress {
            is_downloading: false,
            component_name: String::new(),
            component_id: String::new(),
            url: String::new(),
            downloaded_bytes: 0,
            total_bytes: None,
            percentage: 0.0,
            stage: DownloadStage::Downloading,
            message: err_msg.clone(),
        });
        Err(err_msg)
    }
}

/// Helper for backwards compatibility across existing callers
pub async fn ensure_all_mandatory_components_with_progress<F>(progress_fn: F) -> Result<(), String>
where
    F: FnMut(DownloadProgress),
{
    ensure_mandatory_components_cached_with_progress(progress_fn).await
}

/// Helper to ensure DLSS Mip Fix companion addon is cached
pub async fn ensure_dlss5_d3d12_fix_addon(_log: &mut Vec<String>) -> Result<PathBuf, String> {
    install_or_update_addon(AddonId::DlssMipFix, false, |_| {}).await?;
    let mut roots = vec![get_components_root()];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            roots.push(parent.join("components"));
        }
    }
    for root in roots {
        let p = root.join("dlss-mip-fix").join("dlss-mip-fix.addon64");
        if p.is_file() {
            return Ok(p);
        }
        let p2 = root.join("dlss-mip-fix.addon64");
        if p2.is_file() {
            return Ok(p2);
        }
    }
    Err("DLSS Mip Fix addon could not be located".to_string())
}

/// Helper to ensure RenoDX MFG Unlock addon is cached
pub async fn ensure_mfg_v09_addon(_log: &mut Vec<String>) -> Result<PathBuf, String> {
    install_or_update_addon(AddonId::RenoDxMfgUnlock, false, |_| {}).await?;
    let mut roots = vec![get_components_root()];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            roots.push(parent.join("components"));
        }
    }
    for root in roots {
        let p = root.join("mfg-unlock-1.0").join("renodx-mfgunlock.addon64");
        if p.is_file() {
            return Ok(p);
        }
        let p2 = root.join("renodx-mfgunlock.addon64");
        if p2.is_file() {
            return Ok(p2);
        }
    }
    Err("RenoDX MFG Unlock addon could not be located".to_string())
}

/// Helper for backwards compatibility
pub async fn ensure_feeder_components(log: &mut Vec<String>) -> Result<FeederComponents, String> {
    if let Some(local) = find_local_feeder_components() {
        log.push(format!("[FEEDER-CACHE] Using verified local feeder components from {}", local.addon64.parent().unwrap_or(&local.addon64).display()));
        return Ok(local);
    }

    let comp_root = get_components_root();
    let info = check_addon_update(AddonId::Dlss5Feeder).await;
    let dl_url = info.download_url.unwrap_or_else(|| FEEDER_ARCHIVE_URL.to_string());
    let tag = info.latest_version.unwrap_or_else(|| "1.16.0-beta.3".to_string());
    let tag_clean = tag.trim_start_matches('v');
    let feeder_dir = comp_root.join(format!("DLSS5-Feeder-{}", tag_clean));
    let feeder_zip = comp_root.join(format!("DLSS5-Feeder-{}.zip", tag_clean));

    // 1. Download & extract DLSS5-Feeder
    if !feeder_dir.join("dlss5-feed.addon64").is_file() {
        log.push(format!("[DOWNLOAD] Fetching latest DLSS5-Feeder ({}) from upstream GitHub...", tag));
        download_file_with_progress(&dl_url, &feeder_zip, "", &format!("DLSS 5 Feeder ({})", tag), "feeder", |_| {}).await?;
        log.push("[DOWNLOAD] DLSS5-Feeder downloaded successfully".to_string());

        let file = fs::File::open(&feeder_zip).map_err(|e| format!("Failed to open {}: {}", feeder_zip.display(), e))?;
        extract_zip(file, &feeder_dir)?;
        let _ = fs::remove_file(&feeder_zip);
        let _ = record_addon_version(AddonId::Dlss5Feeder, &tag);
        log.push("[FEEDER] DLSS5-Feeder unpacked successfully".to_string());
    }

    // 2. Download & extract vort_Shaders
    let vort_dir = comp_root.join("vort_Shaders-b410b9f");
    let vort_zip = comp_root.join("vort_Shaders-b410b9f.zip");
    if !vort_dir.join("Shaders").join("vort_Motion.fx").is_file() {
        log.push("[DOWNLOAD] Fetching latest VORT Motion Vector shaders from upstream...".to_string());
        download_file_with_sha256(VORT_ARCHIVE_URL, &vort_zip, VORT_ARCHIVE_SHA256).await?;
        log.push("[DOWNLOAD] Verifying VORT Shaders SHA-256 checksum: OK".to_string());

        let file = fs::File::open(&vort_zip).map_err(|e| format!("Failed to open {}: {}", vort_zip.display(), e))?;
        extract_zip(file, &vort_dir)?;
        let _ = fs::remove_file(&vort_zip);
    }

    // 3. Download ReShade standard framework headers and textures
    let slim_dir = comp_root.join("reshade-shaders-slim");
    let slim_zip = comp_root.join("reshade-shaders-slim.zip");
    if !slim_dir.join("reshade-shaders-6db142b4b1a05c764222e5b0bd9a644b7ccfe1dc").join("Shaders").join("DrawText.fxh").is_file()
        && !slim_dir.join("Shaders").join("DrawText.fxh").is_file()
    {
        log.push("[DOWNLOAD] Fetching ReShade framework headers and textures from upstream slim branch...".to_string());
        if download_file_with_sha256(RESHADE_SHADERS_SLIM_URL, &slim_zip, RESHADE_SHADERS_SLIM_SHA256).await.is_ok() {
            if let Ok(file) = fs::File::open(&slim_zip) {
                let _ = extract_zip(file, &slim_dir);
                let _ = fs::remove_file(&slim_zip);
                log.push("[FEEDER] ReShade framework headers unpacked successfully".to_string());
            }
        }
    }

    let slim_sub = if slim_dir.join("reshade-shaders-6db142b4b1a05c764222e5b0bd9a644b7ccfe1dc").is_dir() {
        slim_dir.join("reshade-shaders-6db142b4b1a05c764222e5b0bd9a644b7ccfe1dc")
    } else {
        slim_dir.clone()
    };

    // 4. Assemble consolidated reshade-shaders tree with full parity to v2.0.1
    let consolidated_shaders = comp_root.join("feeder-shaders");
    let target_shaders = consolidated_shaders.join("Shaders");
    let target_textures = consolidated_shaders.join("Textures");
    let target_includes = target_shaders.join("Includes");
    fs::create_dir_all(&target_includes).map_err(|e| format!("Failed to create {}: {}", target_includes.display(), e))?;
    fs::create_dir_all(&target_textures).map_err(|e| format!("Failed to create {}: {}", target_textures.display(), e))?;

    // Copy DLSS5_Feed.fx from feeder
    let dlss5_feed_fx = feeder_dir.join("reshade-shaders").join("Shaders").join("DLSS5_Feed.fx");
    if dlss5_feed_fx.is_file() {
        let _ = fs::copy(&dlss5_feed_fx, target_shaders.join("DLSS5_Feed.fx"));
    }

    // Copy all framework headers and textures from slim bundle
    if slim_sub.join("Shaders").is_dir() {
        for entry in walkdir::WalkDir::new(slim_sub.join("Shaders")).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.ends_with(".fxh") || fname.ends_with(".fx") {
                    let _ = fs::copy(entry.path(), target_shaders.join(&fname));
                }
            }
        }
    }
    if slim_sub.join("Textures").is_dir() {
        for entry in walkdir::WalkDir::new(slim_sub.join("Textures")).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let fname = entry.file_name().to_string_lossy().to_string();
                let _ = fs::copy(entry.path(), target_textures.join(fname));
            }
        }
    }

    // Fallback direct header copies
    let headers_dir = comp_root.join("reshade-headers");
    let fxh_path = headers_dir.join("ReShade.fxh");
    let ui_fxh_path = headers_dir.join("ReShadeUI.fxh");
    let drawtext_path = headers_dir.join("DrawText.fxh");
    let fontatlas_path = headers_dir.join("FontAtlas.png");

    if fxh_path.is_file() {
        let _ = fs::copy(&fxh_path, target_shaders.join("ReShade.fxh"));
    }
    if ui_fxh_path.is_file() {
        let _ = fs::copy(&ui_fxh_path, target_shaders.join("ReShadeUI.fxh"));
    }
    if drawtext_path.is_file() {
        let _ = fs::copy(&drawtext_path, target_shaders.join("DrawText.fxh"));
    }
    if fontatlas_path.is_file() {
        let _ = fs::copy(&fontatlas_path, target_textures.join("FontAtlas.png"));
    }

    // Copy VORT motion shaders and includes
    let vort_sub = if vort_dir.join("vort_Shaders-b410b9f0c0fbb83c8cb42164aaf1655fab386f4a").is_dir() {
        vort_dir.join("vort_Shaders-b410b9f0c0fbb83c8cb42164aaf1655fab386f4a")
    } else {
        vort_dir.clone()
    };

    if vort_sub.join("Shaders").is_dir() {
        for entry in walkdir::WalkDir::new(vort_sub.join("Shaders")).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.ends_with(".fx") || fname.ends_with(".fxh") {
                    let dest = if entry.path().parent().map(|p| p.ends_with("Includes")).unwrap_or(false) {
                        target_includes.join(&fname)
                    } else {
                        target_shaders.join(&fname)
                    };
                    let _ = fs::copy(entry.path(), dest);
                }
            }
        }
    }

    // Restores vort_BlueNoise.png and all noise textures
    if vort_sub.join("Textures").is_dir() {
        for entry in walkdir::WalkDir::new(vort_sub.join("Textures")).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let fname = entry.file_name().to_string_lossy().to_string();
                let _ = fs::copy(entry.path(), target_textures.join(fname));
            }
        }
    }

    find_local_feeder_components().ok_or_else(|| "DLSS 5 Feeder components could not be located after download".to_string())
}

/// Helper for backwards compatibility
pub async fn ensure_dgvoodoo_components(_log: &mut Vec<String>) -> Result<DgVoodooComponents, String> {
    if let Some(local) = find_local_dgvoodoo_components() {
        return Ok(local);
    }
    install_or_update_addon(AddonId::DgVoodoo, false, |_| {}).await?;
    find_local_dgvoodoo_components().ok_or_else(|| "dgVoodoo2 components could not be located after download".to_string())
}
