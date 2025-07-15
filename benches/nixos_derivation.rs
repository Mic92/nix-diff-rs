use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use nix_diff::{diff::DiffContext, parser, types::DiffOrientation};
use std::path::PathBuf;
use std::process::Command;
use anyhow::Context;

fn generate_nixos_derivations() -> (PathBuf, PathBuf) {
    // Create two slightly different NixOS configurations
    let config1 = r#"
    { config, pkgs, ... }:
    {
      boot.loader.systemd-boot.enable = true;
      boot.loader.efi.canTouchEfiVariables = true;
      
      networking.hostName = "nixos-test";
      networking.networkmanager.enable = true;
      
      time.timeZone = "UTC";
      
      users.users.test = {
        isNormalUser = true;
        extraGroups = [ "wheel" "networkmanager" ];
      };
      
      environment.systemPackages = with pkgs; [
        vim
        git
        htop
      ];
      
      services.openssh.enable = true;
      
      system.stateVersion = "23.11";
    }
    "#;

    let config2 = r#"
    { config, pkgs, ... }:
    {
      boot.loader.systemd-boot.enable = true;
      boot.loader.efi.canTouchEfiVariables = true;
      
      networking.hostName = "nixos-test-v2";
      networking.networkmanager.enable = true;
      
      time.timeZone = "Europe/London";
      
      users.users.test = {
        isNormalUser = true;
        extraGroups = [ "wheel" "networkmanager" "docker" ];
      };
      
      environment.systemPackages = with pkgs; [
        vim
        git
        htop
        tmux
        ripgrep
      ];
      
      services.openssh.enable = true;
      services.openssh.settings.PermitRootLogin = "no";
      
      virtualisation.docker.enable = true;
      
      system.stateVersion = "23.11";
    }
    "#;

    // Write configs to temp files
    let dir = std::env::temp_dir();
    let config1_path = dir.join("nixos-config1.nix");
    let config2_path = dir.join("nixos-config2.nix");

    std::fs::write(&config1_path, config1).unwrap();
    std::fs::write(&config2_path, config2).unwrap();

    // Generate derivations using nix-instantiate
    let output1 = Command::new("nix-instantiate")
        .args([
            "<nixpkgs/nixos>",
            "-A",
            "system",
            "--arg",
            "configuration",
            &config1_path.to_string_lossy(),
        ])
        .output()
        .expect("Failed to run nix-instantiate for config1");

    if !output1.status.success() {
        panic!(
            "nix-instantiate failed for config1:\nstderr: {}\nstdout: {}",
            String::from_utf8_lossy(&output1.stderr),
            String::from_utf8_lossy(&output1.stdout)
        );
    }

    let output2 = Command::new("nix-instantiate")
        .args([
            "<nixpkgs/nixos>",
            "-A",
            "system",
            "--arg",
            "configuration",
            &config2_path.to_string_lossy(),
        ])
        .output()
        .expect("Failed to run nix-instantiate for config2");

    if !output2.status.success() {
        panic!(
            "nix-instantiate failed for config2:\nstderr: {}\nstdout: {}",
            String::from_utf8_lossy(&output2.stderr),
            String::from_utf8_lossy(&output2.stdout)
        );
    }

    let drv1 = PathBuf::from(String::from_utf8_lossy(&output1.stdout).trim());
    let drv2 = PathBuf::from(String::from_utf8_lossy(&output2.stdout).trim());
    
    // Ensure the paths are not empty
    if drv1.as_os_str().is_empty() {
        panic!("nix-instantiate returned empty path for config1");
    }
    if drv2.as_os_str().is_empty() {
        panic!("nix-instantiate returned empty path for config2");
    }

    (drv1, drv2)
}

fn benchmark_nixos_diff(c: &mut Criterion) {
    let (drv1_path, drv2_path) = generate_nixos_derivations();

    c.bench_function("nixos_derivation_parse", |b| {
        b.iter(|| {
            let drv1 = parser::parse_derivation(black_box(&drv1_path))
                .with_context(|| format!("Failed to parse derivation: {}", drv1_path.display()))
                .unwrap();
            let drv2 = parser::parse_derivation(black_box(&drv2_path))
                .with_context(|| format!("Failed to parse derivation: {}", drv2_path.display()))
                .unwrap();
            (drv1, drv2)
        })
    });

    c.bench_function("nixos_derivation_diff", |b| {
        let drv1 = parser::parse_derivation(&drv1_path)
            .with_context(|| format!("Failed to parse derivation: {}", drv1_path.display()))
            .unwrap();
        let drv2 = parser::parse_derivation(&drv2_path)
            .with_context(|| format!("Failed to parse derivation: {}", drv2_path.display()))
            .unwrap();

        b.iter(|| {
            let mut context = DiffContext::new(DiffOrientation::Line, 3);
            context
                .diff_derivations(
                    black_box(&drv1_path),
                    black_box(&drv2_path),
                    black_box(&drv1),
                    black_box(&drv2),
                )
                .unwrap()
        })
    });
}

criterion_group!(benches, benchmark_nixos_diff);
criterion_main!(benches);
