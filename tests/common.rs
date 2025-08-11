use tempfile::TempDir;

pub fn setup_nix_env() -> (TempDir, Vec<(String, String)>) {
    let nix_root = TempDir::new().expect("Failed to create temp dir");
    let nix_root_path = nix_root.path();

    // Create required directories
    std::fs::create_dir_all(nix_root_path.join("store")).expect("Failed to create store dir");
    std::fs::create_dir_all(nix_root_path.join("var/log/nix/drvs"))
        .expect("Failed to create log dir");
    std::fs::create_dir_all(nix_root_path.join("var/nix/profiles"))
        .expect("Failed to create profiles dir");
    std::fs::create_dir_all(nix_root_path.join("state")).expect("Failed to create state dir");
    std::fs::create_dir_all(nix_root_path.join("cache")).expect("Failed to create cache dir");

    let env_vars = vec![
        (
            "NIX_STORE_DIR".to_string(),
            nix_root_path.join("store").to_string_lossy().to_string(),
        ),
        (
            "NIX_DATA_DIR".to_string(),
            nix_root_path.join("share").to_string_lossy().to_string(),
        ),
        (
            "NIX_LOG_DIR".to_string(),
            nix_root_path
                .join("var/log/nix")
                .to_string_lossy()
                .to_string(),
        ),
        (
            "NIX_STATE_DIR".to_string(),
            nix_root_path.join("state").to_string_lossy().to_string(),
        ),
        (
            "NIX_CONF_DIR".to_string(),
            nix_root_path.join("etc").to_string_lossy().to_string(),
        ),
        (
            "XDG_CACHE_HOME".to_string(),
            nix_root_path.join("cache").to_string_lossy().to_string(),
        ),
        (
            "NIX_CONFIG".to_string(),
            "substituters =\nconnect-timeout = 0\nsandbox = false".to_string(),
        ),
        ("_NIX_TEST_NO_SANDBOX".to_string(), "1".to_string()),
        ("NIX_REMOTE".to_string(), "".to_string()),
    ];

    (nix_root, env_vars)
}
