let
  dep1 = builtins.derivation {
    name = "dep1";
    system = builtins.currentSystem;
    builder = "/bin/sh";
    args = ["-c" "mkdir -p $out/bin && echo '#!/bin/sh\necho Dependency 1' > $out/bin/dep1 && chmod +x $out/bin/dep1"];
  };

  dep2 = builtins.derivation {
    name = "dep2";
    system = builtins.currentSystem;
    builder = "/bin/sh";
    args = ["-c" "mkdir -p $out/share && echo 'Shared data v1' > $out/share/data.txt"];
  };
in
  builtins.derivation {
    name = "hello-v1";
    system = builtins.currentSystem;
    builder = "/bin/sh";
    args = [
      "-c"
      ''
            mkdir -p $out/bin
            cat > $out/bin/hello << 'EOF'
        #!/bin/sh
        echo "Hello, World! v1"
        EOF
            chmod +x $out/bin/hello

            # Reference dependencies
            ln -s ${dep1}/bin/dep1 $out/bin/
            ln -s ${dep2}/share $out/
      ''
    ];

    # Environment variables
    version = "1.0";
    description = "A simple hello world program v1";
    license = "MIT";
  }
