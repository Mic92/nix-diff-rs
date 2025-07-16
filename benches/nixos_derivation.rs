#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

use anyhow::Context;
use criterion::{criterion_group, criterion_main, Criterion};
use nix_diff::{diff::DiffContext, parser, types::DiffOrientation};
use std::hint::black_box;
use std::process::Command;

fn generate_nixos_derivations() -> (String, String) {
    // Create two slightly different NixOS configurations
    let config1 = r#"
    { config, pkgs, ... }:
    {
      boot.loader.systemd-boot.enable = true;
      boot.loader.efi.canTouchEfiVariables = true;
      
      fileSystems."/" = {
        device = "/dev/disk/by-label/nixos";
        fsType = "ext4";
      };

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
      
      fileSystems."/" = {
        device = "/dev/disk/by-label/nixos";
        fsType = "ext4";
      };

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

    let drv1 = String::from_utf8_lossy(&output1.stdout).trim().to_string();
    let drv2 = String::from_utf8_lossy(&output2.stdout).trim().to_string();

    // Ensure the paths are not empty
    if drv1.is_empty() {
        panic!("nix-instantiate returned empty path for config1");
    }
    if drv2.is_empty() {
        panic!("nix-instantiate returned empty path for config2");
    }

    (drv1, drv2)
}

fn benchmark_nixos_diff(c: &mut Criterion) {
    let (drv1_path, drv2_path) = generate_nixos_derivations();

    c.bench_function("nixos_derivation_parse", |b| {
        b.iter(|| {
            let drv1 = parser::parse_derivation(black_box(&drv1_path))
                .with_context(|| format!("Failed to parse derivation: {drv1_path}"))
                .unwrap();
            let drv2 = parser::parse_derivation(black_box(&drv2_path))
                .with_context(|| format!("Failed to parse derivation: {drv2_path}"))
                .unwrap();
            (drv1, drv2)
        })
    });

    let mut group = c.benchmark_group("nixos_derivation_diff");
    group.sample_size(30); // Reduce sample size to avoid timeout

    group.bench_function("diff", |b| {
        let drv1 = parser::parse_derivation(&drv1_path)
            .with_context(|| format!("Failed to parse derivation: {drv1_path}"))
            .unwrap();
        let drv2 = parser::parse_derivation(&drv2_path)
            .with_context(|| format!("Failed to parse derivation: {drv2_path}"))
            .unwrap();

        b.iter(|| {
            let mut context = DiffContext::new(DiffOrientation::Line, 3);
            context
                .diff_derivations(
                    black_box(drv1_path.as_bytes()),
                    black_box(drv2_path.as_bytes()),
                    black_box(&drv1),
                    black_box(&drv2),
                )
                .unwrap()
        })
    });
    group.finish();
}

criterion_group!(benches, benchmark_nixos_diff);
criterion_main!(benches);
