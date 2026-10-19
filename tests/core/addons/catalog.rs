use dlss_studio::core::addons::{
    catalog::*,
    AddonId,
    ALL_REFERENCED_ADDONS,
};

#[test]
fn test_addon_id_all_have_metadata() {
    assert_eq!(ALL_REFERENCED_ADDONS.len(), 9);
    for id in ALL_REFERENCED_ADDONS {
        let meta = id.metadata();
        assert_eq!(meta.id, *id);
        assert!(!meta.name.is_empty());
        assert!(!meta.description_key.is_empty());
        assert!(!meta.url.is_empty());
        assert!(!meta.target_filename.is_empty());
    }
}

#[test]
fn test_addon_urls_and_hashes_structure() {
    assert!(FEEDER_ARCHIVE_URL.starts_with("https://"));
    assert_eq!(FEEDER_ARCHIVE_SHA256.len(), 64);

    assert!(MFG_UNLOCK_URL.starts_with("https://"));
    assert_eq!(MFG_UNLOCK_SHA256.len(), 64);

    assert!(DGVOODOO_URL.starts_with("https://"));
    assert_eq!(DGVOODOO_SHA256.len(), 64);

    assert!(RESHADE_SETUP_URL.starts_with("https://"));
    assert_eq!(RESHADE_SETUP_SHA256.len(), 64);

    assert!(OPTISCALER_URL.starts_with("https://github.com/Skynizz/optiscaler-dlss5"));
    assert_eq!(OPTISCALER_SHA256.len(), 64);

    assert_eq!(CDN_BASE_URL, "https://pub-fc0eb49bf297443d8eb1e209c15135fe.r2.dev");
    assert_eq!(RENODX_REPO_URL, CDN_BASE_URL);
    assert_eq!(RENODX_DLSS_URL, "https://pub-fc0eb49bf297443d8eb1e209c15135fe.r2.dev/renodx-dlss.addon64");
    assert_eq!(RENODX_DLSS_SHA256, "");

    assert_eq!(STREAMLINE_REPO_URL, CDN_BASE_URL);
    let streamline_meta = AddonId::Streamline.metadata();
    assert_eq!(streamline_meta.url, STREAMLINE_REPO_URL);
}

#[test]
fn test_renodx_metadata_and_integrity() {
    let meta = AddonId::RenoDxDlss5.metadata();
    assert_eq!(meta.name, "RenoDX (Integrated DLSS 5 Engine)");
    assert_eq!(meta.target_filename, "renodx-dlss.addon64");
    assert_eq!(meta.expected_sha256, RENODX_DLSS_SHA256);
    assert_eq!(meta.url, RENODX_DLSS_URL);
}

#[test]
fn test_optiscaler_asset_matcher_filters_correctly() {
    let matcher = |name: &str| {
        (name.starts_with("OptiScaler-DLSS5-") || name.starts_with("OptiScaler-"))
            && name.ends_with("-win-x64.zip")
            && !name.contains("-source")
    };

    assert!(matcher("OptiScaler-DLSS5-1.2.1-win-x64.zip"));
    assert!(matcher("OptiScaler-1.3.0-win-x64.zip"));
    assert!(!matcher("OptiScaler-DLSS5-1.2.1-source.zip"));
    assert!(!matcher("OptiScaler-DLSS5-1.2.1-win-x64.zip.sha256"));
    assert!(!matcher("random.zip"));
}

#[tokio::test]
async fn test_fallback_resolvers_return_valid_urls() {
    let (url, tag) = resolve_latest_dashdogy_mfg().await;
    assert!(url.starts_with("https://"));
    assert!(!tag.is_empty());

    let (opti_url, opti_tag) = resolve_latest_optiscaler_mfg().await;
    assert!(opti_url.starts_with("https://"));
    assert!(!opti_tag.is_empty());
}

#[test]
fn test_parse_streamline_asset_name() {
    assert_eq!(
        parse_streamline_asset_name("streamline-2.14.1-dlssnr-310.9.16.0.zip"),
        Some("2.14.1 (DLSS-NR 310.9.16.0)".to_string())
    );
    assert_eq!(
        parse_streamline_asset_name("streamline-2.15.0-dlssnr-320.1.0.0.zip"),
        Some("2.15.0 (DLSS-NR 320.1.0.0)".to_string())
    );
    assert_eq!(
        parse_streamline_asset_name("streamline-2.14.1-nr-310.8.0.0.zip"),
        Some("2.14.1 (DLSS-NR 310.8.0.0)".to_string())
    );
    assert_eq!(parse_streamline_asset_name("streamline.zip"), None);
    assert_eq!(parse_streamline_asset_name("random.zip"), None);
    assert_eq!(parse_streamline_asset_name("streamline-.zip"), None);
}

#[test]
fn test_is_newer_version_compound_streamline() {
    use dlss_studio::core::addons::version::is_newer_version;

    // Legacy unbundled v2.14.1 -> bundled compound DLSS-NR
    assert!(is_newer_version("v2.14.1", "2.14.1 (DLSS-NR 310.9.16.0)"));
    assert!(is_newer_version("2.14.1", "2.14.1 (DLSS-NR 310.9.16.0)"));

    // Older DLSS-NR build -> newer DLSS-NR build
    assert!(is_newer_version("2.14.1 (DLSS-NR 310.8.0.0)", "2.14.1 (DLSS-NR 310.9.16.0)"));
    assert!(!is_newer_version("2.14.1 (DLSS-NR 310.9.16.0)", "2.14.1 (DLSS-NR 310.8.0.0)"));

    // Equal compound version
    assert!(!is_newer_version("2.14.1 (DLSS-NR 310.9.16.0)", "2.14.1 (DLSS-NR 310.9.16.0)"));

    // Newer Streamline base version
    assert!(is_newer_version("2.14.1 (DLSS-NR 310.9.16.0)", "2.15.0 (DLSS-NR 310.9.16.0)"));
    assert!(!is_newer_version("2.15.0 (DLSS-NR 310.9.16.0)", "2.14.1 (DLSS-NR 310.9.16.0)"));
}

#[tokio::test]
async fn test_resolve_latest_streamline_cdn() {
    let (url_opt, tag_opt) = resolve_latest_streamline().await;
    // Live Cloudflare CDN has index.json containing streamline-2.14.1-dlssnr-310.9.16.0.zip
    if let (Some(url), Some(tag)) = (url_opt, tag_opt) {
        assert!(url.starts_with(CDN_BASE_URL) || url.contains("streamline"));
        assert!(tag.contains("2.14.1"));
    }
}

#[test]
fn test_is_newer_version_renodx() {
    use dlss_studio::core::addons::version::is_newer_version;
    assert!(is_newer_version("v0.2026.0827.2036", "v0.2026.1016.1704"));
    assert!(!is_newer_version("v0.2026.1016.1704", "v0.2026.0827.2036"));
    assert!(!is_newer_version("v0.2026.1016.1704", "v0.2026.1016.1704"));
}

