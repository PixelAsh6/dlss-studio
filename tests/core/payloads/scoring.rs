use dlss_studio::core::payloads::scoring::*;

#[test]
fn test_score_optiscaler_dir_weights() {
    let score_plain = score_optiscaler_dir("OptiScaler_0.7.0");
    assert!(score_plain > 0);

    let score_higher = score_optiscaler_dir("OptiScaler_0.8.0");
    assert!(score_higher > score_plain);

    let score_dlssnr = score_optiscaler_dir("OptiScaler_0.7.0_dlssnr");
    assert!(score_dlssnr > score_higher);

    let score_mfg = score_optiscaler_dir("OptiScaler_0.7.0_rtx40mfg");
    assert!(score_mfg > score_dlssnr);
}

#[test]
fn test_score_optiscaler_dir_semver() {
    let score_patch = score_optiscaler_dir("OptiScaler_0.7.1");
    let score_base = score_optiscaler_dir("OptiScaler_0.7.0");
    assert!(score_patch > score_base);
}

#[test]
fn test_score_optiscaler_dir_v1_dlss5_precedence() {
    let score_v121 = score_optiscaler_dir("OptiScaler-1.2.1-dlss5");
    let score_v0891_mfg = score_optiscaler_dir("OptiScaler-0.8.91-rtx40-mfg");
    let score_v085_mfg = score_optiscaler_dir("OptiScaler-NR-v0.8.5-rtx40-mfg");

    assert!(score_v121 > score_v0891_mfg, "v1.2.1 dlss5 must score higher than 0.8.91 rtx40-mfg");
    assert!(score_v121 > score_v085_mfg, "v1.2.1 dlss5 must score higher than 0.8.5 rtx40-mfg");
}

