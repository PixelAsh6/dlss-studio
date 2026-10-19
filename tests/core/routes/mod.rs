pub mod common;
pub mod optiscaler;
pub mod native;
pub mod feeder;

use dlss_studio::core::routes::*;
use dlss_studio::core::advisories::collect_advisories;
use dlss_studio::core::scan::GameEntry;
use std::path::PathBuf;

fn make_test_game(api: &str, bitness: u32, dlss: Option<&str>) -> GameEntry {
    GameEntry {
        name: "Test Game".to_string(),
        dir: PathBuf::from("C:\\Games\\Test"),
        exe_path: PathBuf::from("C:\\Games\\Test\\game.exe"),
        exe_rel: "game.exe".to_string(),
        bitness,
        api: api.to_string(),
        dlss_version: dlss.map(|s| s.to_string()),
        ..Default::default()
    }
}

#[test]
fn test_route_factory_all_returns_all_registered_routes() {
    let all = RouteFactory::all();
    assert_eq!(all.len(), 3);
    let ids: Vec<&str> = all.iter().map(|r| r.id()).collect();
    assert!(ids.contains(&"optiscaler"));
    assert!(ids.contains(&"native"));
    assert!(ids.contains(&"feeder"));

    for route in all {
        assert!(!route.id().is_empty());
        assert!(!route.display_name().is_empty());
        assert!(!route.backend().is_empty());
    }
}

#[test]
fn test_route_factory_create_by_id_and_case_insensitivity() {
    assert!(RouteFactory::create("optiscaler").is_some());
    assert!(RouteFactory::create("OptiScaler").is_some());
    assert!(RouteFactory::create("native").is_some());
    assert!(RouteFactory::create("NATIVE").is_some());
    assert!(RouteFactory::create("feeder").is_some());
    assert!(RouteFactory::create("Feeder").is_some());
    assert!(RouteFactory::create("nonexistent_route").is_none());
}

#[test]
fn test_route_factory_create_from_selection() {
    let opti = RouteFactory::create_from_selection("optiscaler", "any").expect("Must resolve optiscaler");
    assert_eq!(opti.id(), "optiscaler");

    let nat = RouteFactory::create_from_selection("reshade", "native").expect("Must resolve native");
    assert_eq!(nat.id(), "native");

    let feeder = RouteFactory::create_from_selection("reshade", "feeder").expect("Must resolve feeder");
    assert_eq!(feeder.id(), "feeder");
}

#[test]
fn test_route_trait_evaluate_advisory_delegation() {
    let opti_route = RouteFactory::create("optiscaler").unwrap();
    let native_route = RouteFactory::create("native").unwrap();
    let feeder_route = RouteFactory::create("feeder").unwrap();

    let g_dx11 = make_test_game("DirectX 11", 64, None);
    assert!(opti_route.evaluate_advisory(&g_dx11).is_some());
    assert!(native_route.evaluate_advisory(&g_dx11).is_some());
    assert!(feeder_route.evaluate_advisory(&g_dx11).is_none());
}

#[test]
fn test_collect_advisories_aggregation() {
    let g = make_test_game("DirectX 11", 64, None);
    let advs_opti = collect_advisories(&g, "optiscaler", "any", false, true);
    assert!(!advs_opti.is_empty(), "OptiScaler on pure DX11 must return route advisory");

    let advs_opti_mfg = collect_advisories(&g, "optiscaler", "any", true, true);
    assert!(advs_opti_mfg.len() >= advs_opti.len(), "Enabling MFG should add MFG advisory if applicable");
}

#[test]
fn test_32bit_game_rejects_optiscaler() {
    let g = make_test_game("DirectX 12", 32, Some("3.7.0"));
    assert_eq!(check_opti_reason(&g), Some(OptiReason::Unsupported));
    let routes = routes_for(&g);
    assert!(!routes.contains(&InstallRoute::OptiScaler));
    assert_eq!(routes, vec![InstallRoute::Feeder]);
}

#[test]
fn test_game_without_dlss_rejects_optiscaler() {
    let g = make_test_game("DirectX 12", 64, None);
    assert_eq!(check_opti_reason(&g), Some(OptiReason::NeedsDlss));
    let routes = routes_for(&g);
    assert!(!routes.contains(&InstallRoute::OptiScaler));
    assert_eq!(routes, vec![InstallRoute::Feeder]);
}

#[test]
fn test_64bit_game_with_dlss_allows_optiscaler() {
    let g = make_test_game("DirectX 12", 64, Some("3.7.0"));
    assert_eq!(check_opti_reason(&g), None);
    let routes = routes_for(&g);
    assert!(routes.contains(&InstallRoute::OptiScaler));
}

#[test]
fn test_dx9_game_routing() {
    let g = make_test_game("DirectX 9", 32, None);
    let routes = routes_for(&g);
    assert_eq!(routes, vec![InstallRoute::Feeder]);
    assert_eq!(recommended_route(&g), InstallRoute::Feeder);
}

#[test]
fn test_dx8_game_routing() {
    let g = make_test_game("DirectX 8", 32, None);
    let routes = routes_for(&g);
    assert_eq!(routes, vec![InstallRoute::Feeder]);
    assert_eq!(recommended_route(&g), InstallRoute::Feeder);
}

#[test]
fn test_vulkan_with_dlss_routing() {
    let g = make_test_game("Vulkan", 64, Some("3.7.0"));
    let routes = routes_for(&g);
    assert!(routes.contains(&InstallRoute::OptiScaler));
    assert!(routes.contains(&InstallRoute::Feeder));
    assert!(!routes.contains(&InstallRoute::Native));
    assert_eq!(recommended_route(&g), InstallRoute::Feeder);
}

#[test]
fn test_dx12_game_with_dlss_allows_all() {
    let g = make_test_game("DirectX 12", 64, Some("3.7.0"));
    assert_eq!(check_opti_reason(&g), None);
    let routes = routes_for(&g);
    assert!(routes.contains(&InstallRoute::Native));
    assert!(routes.contains(&InstallRoute::Feeder));
    assert!(routes.contains(&InstallRoute::OptiScaler));
    assert_eq!(recommended_route(&g), InstallRoute::Native);
    assert!(is_native_dlss_supported(&g));
}

#[test]
fn test_vulkan_and_dx11_games_reject_native_dlss() {
    let g_vk = make_test_game("Vulkan", 64, Some("2.4.2"));
    assert!(!is_native_dlss_supported(&g_vk), "Vulkan games cannot use D3D12 Native DLSS");

    let g_dx11 = make_test_game("DirectX 11", 64, Some("2.4.2"));
    assert!(!is_native_dlss_supported(&g_dx11), "DX11 games cannot use D3D12 Native DLSS");

    let g_nodlss = make_test_game("DirectX 12", 64, None);
    assert!(!is_native_dlss_supported(&g_nodlss), "Games without DLSS cannot use Native DLSS");
}

#[test]
fn test_install_route_and_opti_reason_methods() {
    assert_eq!(InstallRoute::Native.as_str(), "native");
    assert_eq!(InstallRoute::OptiScaler.as_str(), "optiscaler");
    assert_eq!(InstallRoute::Feeder.as_str(), "feeder");

    assert!(OptiReason::Unsupported.message().contains("64-bit"));
    assert!(OptiReason::NeedsDlss.message().contains("original DLSS"));
}

#[test]
fn test_dispatch_deploy_gating_missing_nvngx_dlssnr_fails_across_all_routes() {
    let opts = DeployOptions {
        game_name: Some("Test Game".to_string()),
        game_dir: PathBuf::from("C:\\Games\\Test"),
        exe_path: PathBuf::from("C:\\Games\\Test\\game.exe"),
        api: "DirectX 12".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let empty_payloads = dlss_studio::core::payloads::PayloadBundle {
        optiscaler_dll: PathBuf::from("C:\\nonexistent\\OptiScaler.dll"),
        optiscaler_ini: PathBuf::from("C:\\nonexistent\\OptiScaler.ini"),
        optiscaler_dir: None,
        nvngx_dlss_dll: None,
        nvngx_dlssnr_dll: None, // Missing!
        nvngx_snippet_dll: None,
        rtxmfg_dll: None,
        reshade64_dll: None,
        reshade32_dll: None,
        renodx_dlss5_addon: None,
        dlss5_d3d12_fix_addon: None,
        renodx_mfgunlock_addon: None,
        streamline_dir: None,
        feeder_components: None,
        dgvoodoo: None,
    };

    let expected_err = "Deployment failed: nvngx_dlssnr.dll is missing from components/streamline/.";

    let opti_res = RouteFactory::dispatch_deploy("optiscaler", "any", &opts, &empty_payloads);
    assert_eq!(opti_res.unwrap_err(), expected_err);

    let native_res = RouteFactory::dispatch_deploy("reshade", "native", &opts, &empty_payloads);
    assert_eq!(native_res.unwrap_err(), expected_err);

    let feeder_res = RouteFactory::dispatch_deploy("reshade", "feeder", &opts, &empty_payloads);
    assert_eq!(feeder_res.unwrap_err(), expected_err);
}

#[test]
fn test_is_dlssnr_cached_and_streamline_complete() {
    let _ = dlss_studio::core::addons::is_dlssnr_cached();
    let _ = dlss_studio::core::addons::is_streamline_complete();
}
