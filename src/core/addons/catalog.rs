use super::AddonId;

pub const DLSS5_FEEDER_RELEASE_URL: &str = FEEDER_ARCHIVE_URL;
pub const FEEDER_ARCHIVE_URL: &str = "https://github.com/jlrouzies-fr/DLSS5-Feeder/releases/download/v1.16.0-beta.3/DLSS5-Feeder-1.16.0-beta.3.zip";
pub const FEEDER_ARCHIVE_SHA256: &str = "f2560fe2ef1521451dbe5c48b1bfb91eb0023a10ae57d23d8eb5363ec946ee99";

pub const VORT_ARCHIVE_URL: &str = "https://codeload.github.com/vortigern11/vort_Shaders/zip/b410b9f0c0fbb83c8cb42164aaf1655fab386f4a";
pub const VORT_ARCHIVE_SHA256: &str = "231ba34a75556f9943e359559a89b0d0cc2caa322d9dcdee5630061bf9fe13b6";

pub const RESHADE_SHADERS_SLIM_URL: &str = "https://github.com/crosire/reshade-shaders/archive/6db142b4b1a05c764222e5b0bd9a644b7ccfe1dc.zip";
pub const RESHADE_SHADERS_SLIM_SHA256: &str = "63d76e33ea2ce683d789e0238515570020bc8fbc5f6b215865e94b4e72ce9f1d";

pub const RESHADE_FXH_URL: &str = "https://raw.githubusercontent.com/crosire/reshade-shaders/slim/Shaders/ReShade.fxh";
pub const RESHADE_FXH_SHA256: &str = "c2538cb887a71fec739ec308ff28189ae4e2c8be373b5eb4b988f01c385ad2f7";

pub const RESHADE_UI_FXH_URL: &str = "https://raw.githubusercontent.com/crosire/reshade-shaders/slim/Shaders/ReShadeUI.fxh";
pub const RESHADE_UI_FXH_SHA256: &str = "8db2e5d95e263d91cf0fc5b69c735d64eb0766324e9334861cb7e5d8ff66ec48";

pub const DRAWTEXT_FXH_URL: &str = "https://raw.githubusercontent.com/crosire/reshade-shaders/slim/Shaders/DrawText.fxh";
pub const DRAWTEXT_FXH_SHA256: &str = "b79cc4dfb3e98bcf4c06193d00ea7631d74f467f73a4deeeee13e71336d3e680";

pub const FONTATLAS_PNG_URL: &str = "https://raw.githubusercontent.com/crosire/reshade-shaders/slim/Textures/FontAtlas.png";
pub const FONTATLAS_PNG_SHA256: &str = "11a711a8167d1c1606892e6fa6f661a477e749d6cbdb1ff700ac381842066ec3";

pub const MFG_UNLOCK_URL: &str = "https://github.com/mavismmg/MFGAdaUnlock-RenoDx/releases/download/1.0/renodx-mfgunlock.addon64";
pub const MFG_UNLOCK_SHA256: &str = "f9f10c685e3e89077f751df2394a1629615a56b58d111dff26b39894e772d50e";

pub const RTX40MFG_URL: &str = RTX40MFG_FALLBACK_URL;
pub const RTX40MFG_FALLBACK_URL: &str = "https://github.com/dashdogy/RTX40MFG-Unlock/releases/download/v1.3.3-hotfix.2/RTXMFG-v1.3.3-hotfix.2.zip";
pub const OPTISCALER_REPO: &str = "Skynizz/optiscaler-dlss5";
pub const OPTISCALER_REPO_URL: &str = "https://github.com/Skynizz/optiscaler-dlss5";
pub const OPTISCALER_URL: &str = "https://github.com/Skynizz/optiscaler-dlss5/releases/download/v1.2.1/OptiScaler-DLSS5-1.2.1-win-x64.zip";
pub const OPTISCALER_SHA256: &str = "0d5b24887ea2c622754da68efc76ef55ae42b5d3be350a8d43cafbabb6747ec6";
pub const OPTISCALER_FALLBACK_URL: &str = OPTISCALER_URL;
pub const OPTISCALER_MFG_FALLBACK_URL: &str = OPTISCALER_URL;

pub const DLSS_MIP_FIX_URL: &str = "https://github.com/bookamp/dlss-studio/releases/latest/download/dlss-mip-fix.addon64";
pub const DLSS_NR_URL: &str = "https://github.com/bookamp/dlss-studio/releases/latest/download/dlss-nr.addon64";

pub const CDN_BASE_URL: &str = "https://pub-fc0eb49bf297443d8eb1e209c15135fe.r2.dev";
pub const RENODX_REPO_URL: &str = CDN_BASE_URL;
pub const RENODX_DLSS_URL: &str = "https://pub-fc0eb49bf297443d8eb1e209c15135fe.r2.dev/renodx-dlss.addon64";
pub const RENODX_DLSS_SHA256: &str = "";
pub const RENODX_DLSS5_URL: &str = RENODX_DLSS_URL;
pub const RENODX_DLSS5_SHA256: &str = RENODX_DLSS_SHA256;
pub const STREAMLINE_REPO_URL: &str = CDN_BASE_URL;
pub const NVNGX_DLSSNR_URL: &str = "https://pub-fc0eb49bf297443d8eb1e209c15135fe.r2.dev/nvngx_dlssnr.dll";
pub const RESHADE_SETUP_URL: &str = "https://reshade.me/downloads/ReShade_Setup_6.8.0_Addon.exe";
pub const RESHADE_SETUP_SHA256: &str = "afe4c8f13048306307983b8b3d41d5bf00a86820440b0e57dea10950e1176445";

pub const DGVOODOO_URL: &str = "https://github.com/dege-diosg/dgVoodoo2/releases/download/v2.87.5/dgVoodoo2_87_5.zip";
pub const DGVOODOO_SHA256: &str = "5ffde6927f7355ca3fdd5d785b581256a8e6539fa13e395a891ade6ba1040850";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddonMetadata {
    pub id: AddonId,
    pub name: &'static str,
    pub description_key: &'static str,
    pub url: &'static str,
    pub expected_sha256: &'static str,
    pub mandatory: bool,
    pub target_filename: &'static str,
}

pub fn get_metadata(id: AddonId) -> AddonMetadata {
    match id {
        AddonId::RenoDxDlss5 => AddonMetadata {
            id,
            name: "RenoDX (Integrated DLSS 5 Engine)",
            description_key: "addon_renodx_desc",
            url: RENODX_DLSS_URL,
            expected_sha256: RENODX_DLSS_SHA256,
            mandatory: true,
            target_filename: "renodx-dlss.addon64",
        },
        AddonId::DlssMipFix => AddonMetadata {
            id,
            name: "DLSS Studio D3D12 Mip Companion",
            description_key: "addon_dlss5_d3d12_fix_desc",
            url: DLSS_MIP_FIX_URL,
            expected_sha256: "",
            mandatory: true,
            target_filename: "dlss-mip-fix.addon64",
        },
        AddonId::RenoDxMfgUnlock => AddonMetadata {
            id,
            name: "RenoDX 4x MFG Unlock v1.0",
            description_key: "addon_mfg_desc",
            url: MFG_UNLOCK_URL,
            expected_sha256: MFG_UNLOCK_SHA256,
            mandatory: true,
            target_filename: "renodx-mfgunlock.addon64",
        },
        AddonId::Dlss5Feeder => AddonMetadata {
            id,
            name: "DLSS 5 Feeder (Neural Pipeline Interceptor)",
            description_key: "addon_feeder_desc",
            url: FEEDER_ARCHIVE_URL,
            expected_sha256: FEEDER_ARCHIVE_SHA256,
            mandatory: true,
            target_filename: "dlss5-feed.addon64",
        },
        AddonId::Rtx40Mfg => AddonMetadata {
            id,
            name: "Universal RTX40MFG-Unlock (Dashdogy)",
            description_key: "tag_rtx40",
            url: RTX40MFG_FALLBACK_URL,
            expected_sha256: "",
            mandatory: true,
            target_filename: "RTXMFG.dll",
        },
        AddonId::OptiScaler => AddonMetadata {
            id,
            name: "OptiScaler DLSS 5 (Edit Cache & Pre-SR)",
            description_key: "addon_optiscaler_desc",
            url: OPTISCALER_URL,
            expected_sha256: OPTISCALER_SHA256,
            mandatory: true,
            target_filename: "OptiScaler.dll",
        },
        AddonId::ReShade => AddonMetadata {
            id,
            name: "ReShade 6.8.0 (Add-on Support)",
            description_key: "addon_reshade_desc",
            url: RESHADE_SETUP_URL,
            expected_sha256: RESHADE_SETUP_SHA256,
            mandatory: true,
            target_filename: "ReShade64.dll",
        },
        AddonId::Streamline => AddonMetadata {
            id,
            name: "Streamline & Neural Rendering (DLSS-NR)",
            description_key: "addon_streamline_desc",
            url: STREAMLINE_REPO_URL,
            expected_sha256: "",
            mandatory: true,
            target_filename: "sl.interposer.dll",
        },
        AddonId::DgVoodoo => AddonMetadata {
            id,
            name: "dgVoodoo2 v2.87.5 (Legacy D3D -> D3D11)",
            description_key: "addon_dgvoodoo_desc",
            url: DGVOODOO_URL,
            expected_sha256: DGVOODOO_SHA256,
            mandatory: true,
            target_filename: "D3D9.dll",
        },
    }
}

/// Queries GitHub API for the latest release asset matching a pattern.
/// Returns (download_url, tag_name) or Err.
pub async fn resolve_github_latest_asset<F>(repo: &str, matcher: F) -> Result<(String, String), String>
where
    F: Fn(&str) -> bool,
{
    let client = reqwest::Client::builder()
        .user_agent(concat!("Mozilla/5.0 (Windows NT 10.0; Win64; x64) DLSS-Studio/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(6))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("https://api.github.com/repos/{}/releases", repo);
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("GitHub API {} returned HTTP {}", url, resp.status()));
    }
    let releases: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;
    for rel in releases {
        let tag = rel.get("tag_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if let Some(assets) = rel.get("assets").and_then(|v| v.as_array()) {
            for asset in assets {
                let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
                if matcher(name) {
                    if let Some(dl_url) = asset.get("browser_download_url").and_then(|v| v.as_str()) {
                        return Ok((dl_url.to_string(), tag));
                    }
                }
            }
        }
    }
    Err("No matching asset found in releases".to_string())
}

/// Resolves the latest download URL and tag for Dashdogy's RTX40MFG-Unlock dynamically
pub async fn resolve_latest_dashdogy_mfg() -> (String, String) {
    if let Ok((url, tag)) = resolve_github_latest_asset("dashdogy/RTX40MFG-Unlock", |name| {
        name.starts_with("RTXMFG-") && name.ends_with(".zip")
    }).await {
        return (url, tag);
    }
    (
        RTX40MFG_FALLBACK_URL.to_string(),
        "v1.3.3-hotfix.2".to_string(),
    )
}

/// Resolves the latest download URL and tag for OptiScaler dynamically
pub async fn resolve_latest_optiscaler() -> (String, String) {
    if let Ok((url, tag)) = resolve_github_latest_asset("Skynizz/optiscaler-dlss5", |name| {
        (name.starts_with("OptiScaler-DLSS5-") || name.starts_with("OptiScaler-"))
            && name.ends_with("-win-x64.zip")
            && !name.contains("-source")
    }).await {
        return (url, tag);
    }
    (
        OPTISCALER_FALLBACK_URL.to_string(),
        "v1.2.1".to_string(),
    )
}

/// Helper alias
pub async fn resolve_latest_optiscaler_mfg() -> (String, String) {
    resolve_latest_optiscaler().await
}

/// Parses the compound version string from a versioned Streamline release asset filename.
/// Expects format: "streamline-{sl_ver}-dlssnr-{nr_ver}.zip"
/// Example: "streamline-2.14.1-dlssnr-310.9.16.0.zip" -> Some("2.14.1 (DLSS-NR 310.9.16.0)")
pub fn parse_streamline_asset_name(name: &str) -> Option<String> {
    let clean = name.trim();
    if !clean.starts_with("streamline-") || !clean.ends_with(".zip") {
        return None;
    }
    let inner = clean.trim_start_matches("streamline-").trim_end_matches(".zip");
    let (sl_ver, nr_ver) = if let Some(idx) = inner.find("-dlssnr-") {
        (&inner[..idx], &inner[idx + "-dlssnr-".len()..])
    } else if let Some(idx) = inner.find("-nr-") {
        (&inner[..idx], &inner[idx + "-nr-".len()..])
    } else {
        return None;
    };
    if sl_ver.is_empty() || nr_ver.is_empty() {
        return None;
    }
    Some(format!("{} (DLSS-NR {})", sl_ver, nr_ver))
}

/// Resolves the latest download URL and dynamic version tag for Streamline & DLSS-NR.
/// Priority 1: Cloudflare R2 CDN index.json for verified community asset bundles.
/// Priority 2: Local staged asset check (for local testing and packaging).
/// Priority 3: Fallback query to GitHub Releases API.
pub async fn resolve_latest_streamline() -> (Option<String>, Option<String>) {
    // 1. Check Cloudflare R2 CDN index.json
    let cdn_index_url = format!("{}/index.json", CDN_BASE_URL);
    if let Ok(client) = reqwest::Client::builder()
        .user_agent(concat!("Mozilla/5.0 (Windows NT 10.0; Win64; x64) DLSS-Studio/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(6))
        .build()
    {
        if let Ok(resp) = client.get(&cdn_index_url).send().await {
            if resp.status().is_success() {
                if let Ok(files) = resp.json::<Vec<String>>().await {
                    let mut candidates = Vec::new();
                    for file in files {
                        if let Some(version_tag) = parse_streamline_asset_name(&file) {
                            let dl_url = format!("{}/{}", CDN_BASE_URL, file);
                            candidates.push((dl_url, version_tag));
                        }
                    }
                    if !candidates.is_empty() {
                        candidates.sort_by(|a, b| {
                            if super::version::is_newer_version(&a.1, &b.1) {
                                std::cmp::Ordering::Less
                            } else if super::version::is_newer_version(&b.1, &a.1) {
                                std::cmp::Ordering::Greater
                            } else {
                                a.1.cmp(&b.1)
                            }
                        });
                        if let Some((url, tag)) = candidates.pop() {
                            return (Some(url), Some(tag));
                        }
                    }
                }
            }
        }
    }

    // 2. Local staged asset check (for local testing before public release)
    let mut local_search_dirs = vec![
        std::path::PathBuf::from("dist"),
        std::path::PathBuf::from("external_assets"),
    ];
    if let Ok(exe_p) = std::env::current_exe() {
        if let Some(parent) = exe_p.parent() {
            local_search_dirs.push(parent.join("dist"));
            local_search_dirs.push(parent.join("external_assets"));
            if let Some(grand) = parent.parent() {
                local_search_dirs.push(grand.join("dist"));
                local_search_dirs.push(grand.join("external_assets"));
            }
        }
    }
    for dir in local_search_dirs {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if let Some(version_tag) = parse_streamline_asset_name(&name) {
                    let full_path = entry.path().to_string_lossy().to_string();
                    return (Some(full_path), Some(version_tag));
                }
            }
        }
    }

    (None, None)
}
