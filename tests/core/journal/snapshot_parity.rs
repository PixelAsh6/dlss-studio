use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use sha2::{Digest, Sha256};
use crate::common::TempDir;
use dlss_studio::core::journal::restore::restore_game;
use dlss_studio::core::routes::common::DeployOptions;
use dlss_studio::core::routes::optiscaler::deploy_optiscaler_with_bundle;
use dlss_studio::core::routes::native::deploy_native_dlss5_with_bundle;
use dlss_studio::core::routes::feeder::deploy_feeder_with_bundle;
use dlss_studio::core::payloads::bundle::PayloadBundle;

/// Recursively computes SHA-256 hashes of all files in a directory,
/// returning a deterministic map of RelativePath -> SHA-256 hex string.
/// Automatically excludes DLSS Studio's backup folder `_DLSS5_Backup`.
fn hash_directory_files(dir: &Path) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for entry in walkdir::WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let rel = entry.path().strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/");
            if rel.starts_with("_DLSS5_Backup/") || rel == "_DLSS5_Backup" {
                continue;
            }
            let bytes = fs::read(entry.path()).expect("file must be readable for hash calculation");
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            let hex_hash = hex::encode(hasher.finalize());
            map.insert(rel, hex_hash);
        }
    }
    map
}

fn create_test_payload_bundle(temp: &Path) -> PayloadBundle {
    let opti_dir = temp.join("mock_opti_payload");
    fs::create_dir_all(opti_dir.join("OptiScaler")).unwrap();
    let optiscaler_dll = opti_dir.join("OptiScaler.dll");
    fs::write(&optiscaler_dll, b"OPTISCALER_PAYLOAD_PE_DLL_BYTES").unwrap();
    let optiscaler_ini = opti_dir.join("OptiScaler.ini");
    fs::write(&optiscaler_ini, b"[DlssNr]\nEnabled=false\n\n[Plugins]\nLoadReshade=true\n\n[FrameGen]\nExternal=false\n").unwrap();

    let nr_dll = temp.join("nvngx_dlssnr.dll");
    fs::write(&nr_dll, b"DLSSNR_PAYLOAD_BYTES").unwrap();

    let rtxmfg = temp.join("RTXMFG.dll");
    fs::write(&rtxmfg, b"RTXMFG_PAYLOAD_BYTES").unwrap();

    let reshade = temp.join("ReShade64.dll");
    fs::write(&reshade, b"RESHADE64_PAYLOAD_BYTES").unwrap();

    let reshade32 = temp.join("ReShade32.dll");
    fs::write(&reshade32, b"RESHADE32_PAYLOAD_BYTES").unwrap();

    let renodx = temp.join("renodx-dlss.addon64");
    fs::write(&renodx, b"RENODX_PAYLOAD_BYTES").unwrap();

    let fix = temp.join("dlss-mip-fix.addon64");
    fs::write(&fix, b"MIPFIX_PAYLOAD_BYTES").unwrap();

    let mfgunlock = temp.join("renodx-mfgunlock.addon64");
    fs::write(&mfgunlock, b"MFGUNLOCK_PAYLOAD_BYTES").unwrap();

    let snippet = temp.join("nvngx.dll_dlssnr.dll");
    fs::write(&snippet, b"SNIPPET_PAYLOAD_BYTES").unwrap();

    let streamline_dir = temp.join("mock_streamline");
    fs::create_dir_all(&streamline_dir).unwrap();
    fs::write(streamline_dir.join("sl.interposer.dll"), b"UPGRADED_SL_INTERPOSER_2_14").unwrap();
    fs::write(streamline_dir.join("sl.common.dll"), b"UPGRADED_SL_COMMON_2_14").unwrap();
    fs::write(streamline_dir.join("sl.dlss_g.dll"), b"UPGRADED_SL_DLSSG_2_14").unwrap();
    fs::write(streamline_dir.join("sl.reflex.dll"), b"UPGRADED_SL_REFLEX_2_14").unwrap();
    fs::write(streamline_dir.join("nvngx_dlssg.dll"), b"UPGRADED_NVNGX_DLSSG_2_14").unwrap();

    let feeder_dir = temp.join("mock_feeder");
    let feeder_shaders = feeder_dir.join("feeder-shaders");
    fs::create_dir_all(feeder_shaders.join("Shaders")).unwrap();
    fs::create_dir_all(feeder_shaders.join("Textures")).unwrap();
    fs::write(feeder_shaders.join("Shaders").join("DLSS5_Feed.fx"), b"FEEDER_FX").unwrap();
    fs::write(feeder_shaders.join("Shaders").join("vort_Motion.fx"), b"VORT_FX").unwrap();
    fs::write(feeder_shaders.join("Textures").join("FontAtlas.png"), b"ATLAS_PNG").unwrap();
    fs::write(feeder_shaders.join("Shaders").join("ReShade.fxh"), b"FXH").unwrap();
    fs::write(feeder_shaders.join("Shaders").join("ReShadeUI.fxh"), b"UI_FXH").unwrap();
    fs::write(feeder_shaders.join("Shaders").join("DrawText.fxh"), b"TEXT_FXH").unwrap();
    let mock_addon64 = feeder_dir.join("dlss5-feed.addon64");
    fs::write(&mock_addon64, b"FEEDER_ADDON").unwrap();

    let host64_dir = feeder_dir.join("host64");
    fs::create_dir_all(&host64_dir).unwrap();
    let mock_host64 = host64_dir.join("dlss5-feed-host64.exe");
    fs::write(&mock_host64, b"HOST64_EXE").unwrap();
    fs::write(host64_dir.join("dxgi.dll"), b"HOST64_DXGI").unwrap();
    fs::write(host64_dir.join("dlss5-feed.addon64"), b"HOST64_ADDON").unwrap();

    let mock_feeder = dlss_studio::core::addons::FeederComponents {
        addon64: mock_addon64,
        addon32: None,
        host64: Some(mock_host64),
        shader_dir: feeder_shaders,
        vk_layer_dir: None,
    };

    PayloadBundle {
        optiscaler_dir: Some(opti_dir),
        optiscaler_dll,
        optiscaler_ini,
        nvngx_dlss_dll: None,
        nvngx_dlssnr_dll: Some(nr_dll),
        nvngx_snippet_dll: Some(snippet),
        rtxmfg_dll: Some(rtxmfg),
        reshade64_dll: Some(reshade),
        reshade32_dll: Some(reshade32),
        renodx_dlss5_addon: Some(renodx),
        dlss5_d3d12_fix_addon: Some(fix),
        renodx_mfgunlock_addon: Some(mfgunlock),
        streamline_dir: Some(streamline_dir),
        feeder_components: Some(mock_feeder),
        dgvoodoo: None,
    }
}

#[test]
fn test_cryptographic_parity_streamline_game_restores_100_percent_byte_identical() {
    let temp = TempDir::new("parity_streamline");
    let game_dir = temp.join("The Witcher 3 Wild Hunt");
    let bin_dir = game_dir.join("bin").join("x64_dx12");
    fs::create_dir_all(&bin_dir).unwrap();

    let exe_path = bin_dir.join("witcher3.exe");
    fs::write(&exe_path, b"ORIGINAL_WITCHER3_BINARY_MZ_DUMMY").unwrap();

    // Genuine CDPR / GOG vanilla Streamline files
    fs::write(bin_dir.join("sl.interposer.dll"), b"ORIGINAL_VANILLA_CDPR_INTERPOSER").unwrap();
    fs::write(bin_dir.join("sl.pcl.dll"), b"ORIGINAL_VANILLA_CDPR_SL_PCL_REFLEX_ANALYZER").unwrap();
    fs::write(bin_dir.join("sl.common.dll"), b"ORIGINAL_VANILLA_CDPR_SL_COMMON").unwrap();
    fs::write(bin_dir.join("sl.dlss_g.dll"), b"ORIGINAL_VANILLA_CDPR_SL_DLSSG").unwrap();
    fs::write(bin_dir.join("sl.reflex.dll"), b"ORIGINAL_VANILLA_CDPR_SL_REFLEX").unwrap();
    fs::write(bin_dir.join("nvngx_dlss.dll"), b"ORIGINAL_VANILLA_NVNGX_DLSS").unwrap();

    // Non-DLL game data assets and custom configs
    fs::write(bin_dir.join("user_settings.ini"), b"[Rendering]\nGamma=1.0\nDLSS=Quality\n").unwrap();
    fs::write(bin_dir.join("witcher3_engine.dat"), b"RAW_ASSET_DATA_BYTES_12345").unwrap();

    // 1. Capture exact cryptographic snapshot of the vanilla game folder BEFORE any modding
    let snapshot_before = hash_directory_files(&game_dir);
    assert_eq!(snapshot_before.len(), 9, "Expected exactly 9 original files in vanilla game folder");

    let payloads = create_test_payload_bundle(&temp.join("payloads"));
    let opts = DeployOptions {
        game_name: Some("The Witcher 3: Wild Hunt".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: true,
        passes: 1,
        mfg_unlock: true,
        mfg_multiplier: 4,
        nr_style: 0,
        nr_style_enabled: false,
    };

    // 2. Deploy OptiScaler route
    let deploy_res = deploy_optiscaler_with_bundle(&opts, &payloads).expect("deploy must succeed");
    assert!(deploy_res.success);

    // Verify mod files are active
    assert!(bin_dir.join("dxgi.dll").exists(), "OptiScaler dxgi.dll must be deployed");
    assert!(bin_dir.join("OptiScaler.ini").exists(), "OptiScaler.ini must be deployed");

    // 3. Execute Restore
    let restore_res = restore_game(&game_dir).expect("restore must succeed");
    assert!(restore_res, "restore must report success");

    // 4. Capture cryptographic snapshot AFTER restore
    let snapshot_after = hash_directory_files(&game_dir);

    // 5. BULLETPROOF ASSERTION: 100.000% identity match
    assert_eq!(
        snapshot_before,
        snapshot_after,
        "Game directory after restore MUST be 100% byte-for-byte identical to the original vanilla directory!"
    );
}

#[test]
fn test_cryptographic_parity_multi_route_swap_survives_all_intermediates() {
    let temp = TempDir::new("parity_multiswap");
    let game_dir = temp.join("Cyberpunk 2077");
    let bin_dir = game_dir.join("bin").join("x64");
    fs::create_dir_all(&bin_dir).unwrap();

    let exe_path = bin_dir.join("Cyberpunk2077.exe");
    fs::write(&exe_path, b"CYBERPUNK_EXE_ORIGINAL").unwrap();
    fs::write(bin_dir.join("sl.interposer.dll"), b"ORIGINAL_CP2077_SL_INTERPOSER").unwrap();
    fs::write(bin_dir.join("sl.pcl.dll"), b"ORIGINAL_CP2077_SL_PCL").unwrap();
    fs::write(bin_dir.join("sl.common.dll"), b"ORIGINAL_CP2077_SL_COMMON").unwrap();
    fs::write(bin_dir.join("nvngx_dlss.dll"), b"ORIGINAL_CP2077_DLSS").unwrap();
    fs::write(bin_dir.join("engine_config.json"), b"{\"Engine\": \"REDengine4\"}").unwrap();

    // 1. Snapshot BEFORE
    let snapshot_before = hash_directory_files(&game_dir);
    assert_eq!(snapshot_before.len(), 6);

    let payloads = create_test_payload_bundle(&temp.join("payloads"));
    let mut opts = DeployOptions {
        game_name: Some("Cyberpunk 2077".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: true,
        passes: 1,
        mfg_unlock: true,
        mfg_multiplier: 4,
        nr_style: 0,
        nr_style_enabled: false,
    };

    // Swap 1: Deploy Native RenoDX
    let res1 = deploy_native_dlss5_with_bundle(&opts, &payloads).expect("deploy native must succeed");
    assert!(res1.success);

    // Swap 2: Hot-swap to OptiScaler
    let res2 = deploy_optiscaler_with_bundle(&opts, &payloads).expect("swap to optiscaler must succeed");
    assert!(res2.success);

    // Swap 3: Hot-swap to Feeder
    let res3 = deploy_feeder_with_bundle(&opts, &payloads).expect("swap to feeder must succeed");
    assert!(res3.success);

    // Swap 4: Hot-swap back to OptiScaler
    opts.mfg_multiplier = 2;
    let res4 = deploy_optiscaler_with_bundle(&opts, &payloads).expect("swap back to optiscaler must succeed");
    assert!(res4.success);

    // Restore back to vanilla
    let restored = restore_game(&game_dir).expect("restore must succeed");
    assert!(restored);

    // Snapshot AFTER
    let snapshot_after = hash_directory_files(&game_dir);

    // 5. BULLETPROOF ASSERTION: Zero files lost, zero files corrupted, zero mod files left behind across 4 swaps!
    assert_eq!(
        snapshot_before,
        snapshot_after,
        "Every single file must be restored 100% byte-for-byte even after 4 consecutive cross-route swaps!"
    );
}

#[test]
fn test_cryptographic_parity_clean_slate_game_leaves_zero_stray_artifacts() {
    let temp = TempDir::new("parity_cleanslate");
    let game_dir = temp.join("SimpleIndieGame");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("IndieGame.exe");
    fs::write(&exe_path, b"INDIE_GAME_MAIN_BINARY").unwrap();
    fs::write(game_dir.join("game_settings.json"), b"{\"resolution\": \"1080p\"}").unwrap();
    fs::write(game_dir.join("save_data.sav"), b"PLAYER_SAVE_DATA_LEVEL_99").unwrap();

    // Snapshot BEFORE
    let snapshot_before = hash_directory_files(&game_dir);
    assert_eq!(snapshot_before.len(), 3);

    let payloads = create_test_payload_bundle(&temp.join("payloads"));
    let opts = DeployOptions {
        game_name: Some("Simple Indie Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: true,
        passes: 1,
        mfg_unlock: true,
        mfg_multiplier: 3,
        nr_style: 0,
        nr_style_enabled: false,
    };

    // Deploy OptiScaler (injects dxgi.dll, version.dll, OptiScaler.ini, OptiScaler/ directory)
    let res = deploy_optiscaler_with_bundle(&opts, &payloads).expect("deploy must succeed");
    assert!(res.success);

    // Verify mod was installed
    assert!(game_dir.join("dxgi.dll").exists());
    assert!(game_dir.join("OptiScaler.ini").exists());

    // Restore
    let restored = restore_game(&game_dir).expect("restore must succeed");
    assert!(restored);

    // Snapshot AFTER
    let snapshot_after = hash_directory_files(&game_dir);

    // All injected files must be completely eradicated; original files match 100%
    assert_eq!(
        snapshot_before,
        snapshot_after,
        "All mod files and directories must be completely purged, leaving only the original game files intact!"
    );
}

#[test]
fn test_cryptographic_parity_with_pre_existing_dxgi_and_custom_dlls() {
    let temp = TempDir::new("parity_preexisting_dlls");
    let game_dir = temp.join("ModdedGame");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("game.exe");
    fs::write(&exe_path, b"GAME_EXE_CONTENT").unwrap();
    // User already had a pre-existing dxgi.dll and version.dll before DLSS Studio was ever launched
    fs::write(game_dir.join("dxgi.dll"), b"USER_PREEXISTING_CUSTOM_DXGI_WRAPPER_BYTES").unwrap();
    fs::write(game_dir.join("version.dll"), b"USER_PREEXISTING_CUSTOM_VERSION_WRAPPER_BYTES").unwrap();
    fs::write(game_dir.join("game_engine.dll"), b"ORIGINAL_GAME_ENGINE_DLL").unwrap();

    let snapshot_before = hash_directory_files(&game_dir);
    assert_eq!(snapshot_before.len(), 4);

    let payloads = create_test_payload_bundle(&temp.join("payloads"));
    let opts = DeployOptions {
        game_name: Some("Modded Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: true,
        passes: 1,
        mfg_unlock: true,
        mfg_multiplier: 2,
        nr_style: 0,
        nr_style_enabled: false,
    };

    // Deploy OptiScaler (replaces dxgi.dll and version.dll with mod versions)
    let res = deploy_optiscaler_with_bundle(&opts, &payloads).expect("deploy must succeed");
    assert!(res.success);

    // Verify mod versions replaced the originals
    assert_ne!(fs::read(game_dir.join("dxgi.dll")).unwrap(), b"USER_PREEXISTING_CUSTOM_DXGI_WRAPPER_BYTES");

    // Restore back to vanilla
    let restored = restore_game(&game_dir).expect("restore must succeed");
    assert!(restored);

    let snapshot_after = hash_directory_files(&game_dir);

    // The user's original dxgi.dll and version.dll MUST be restored 100% byte-for-byte!
    assert_eq!(
        snapshot_before,
        snapshot_after,
        "User's pre-existing dxgi.dll and version.dll must be restored exactly as they were before deployment!"
    );
}

#[test]
fn test_snapshot_parity_baldurs_gate_3_dx11_and_dx12() {
    let temp = TempDir::new("parity_bg3");
    let game_dir = temp.join("Baldurs Gate 3");
    let bin_dir = game_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let exe_dx12 = bin_dir.join("bg3.exe");
    let exe_dx11 = bin_dir.join("bg3_dx11.exe");
    fs::write(&exe_dx12, b"BG3_DX12_EXECUTABLE").unwrap();
    fs::write(&exe_dx11, b"BG3_DX11_EXECUTABLE").unwrap();

    // Vanilla DLSS 2.4 DLL shipped with BG3
    let vanilla_dlss = bin_dir.join("nvngx_dlss.dll");
    fs::write(&vanilla_dlss, b"VANILLA_BG3_NVNGX_DLSS_V2_4_0").unwrap();
    fs::write(bin_dir.join("bink2w64.dll"), b"VANILLA_BINK_VIDEO_PLAYER").unwrap();
    fs::write(bin_dir.join("engine.dll"), b"VANILLA_LSLIB_DIVINITY_ENGINE").unwrap();
    fs::write(bin_dir.join("graphic_options.json"), b"{\"api\": \"dx11\"}").unwrap();

    let snapshot_before = hash_directory_files(&game_dir);
    assert_eq!(snapshot_before.len(), 6);

    let payloads = create_test_payload_bundle(&temp.join("payloads"));
    let opts = DeployOptions {
        game_name: Some("Baldur's Gate 3".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_dx12.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: true,
        passes: 1,
        mfg_unlock: true,
        mfg_multiplier: 2,
        nr_style: 0,
        nr_style_enabled: false,
    };

    // 1. Deploy OptiScaler route (swaps nvngx_dlss.dll and installs OptiScaler)
    let res = deploy_optiscaler_with_bundle(&opts, &payloads).expect("deploy must succeed");
    assert!(res.success);

    // 2. Hot swap to Feeder route
    let opts_feeder = DeployOptions {
        api: "DirectX 11".to_string(),
        exe_path: exe_dx11.clone(),
        ..opts.clone()
    };
    let res_feeder = deploy_feeder_with_bundle(&opts_feeder, &payloads).expect("feeder deploy must succeed");
    assert!(res_feeder.success);

    // 3. Restore to vanilla
    let restored = restore_game(&game_dir).expect("restore must succeed");
    assert!(restored);

    let snapshot_after = hash_directory_files(&game_dir);

    // 4. Assert 100% cryptographic parity: original DLSS 2.4.0 restored, engine intact, all mods purged!
    assert_eq!(
        snapshot_before,
        snapshot_after,
        "Baldur's Gate 3 must restore to 100% byte-for-byte vanilla state after multi-API swaps!"
    );
}
