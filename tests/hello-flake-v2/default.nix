let
  dep1 = builtins.derivation {
    name = "dep1";
    system = builtins.currentSystem;
    builder = "/bin/sh";
    args = ["-c" "mkdir -p $out/bin && echo '#!/bin/sh\necho Dependency 1 updated' > $out/bin/dep1 && chmod +x $out/bin/dep1"];
  };

  dep2 = builtins.derivation {
    name = "dep2";
    system = builtins.currentSystem;
    builder = "/bin/sh";
    args = ["-c" "mkdir -p $out/share && echo 'Shared data v2' > $out/share/data.txt"];
  };
in
  builtins.derivation {
    name = "hello-v2";
    system = builtins.currentSystem;
    builder = "/bin/sh";
    args = [
      "-c"
      ''
            mkdir -p $out/bin
            cat > $out/bin/hello << 'EOF'
        #!/bin/sh
        echo "Hello, World! v2"
        echo "Now with more features!"
        EOF
            chmod +x $out/bin/hello

            # Reference dependencies
            ln -s ${dep1}/bin/dep1 $out/bin/
            ln -s ${dep2}/share $out/
      ''
    ];

    # Environment variables
    version = "2.0";
    description = "A simple hello world program v2 with improvements";
    license = "MIT";
    newFeature = "true";
    buildScript = ''
      echo "Starting build process..."
      echo "Configuring environment"
      echo "Setting up new features"
      echo "Building dependencies"
      echo "Compiling sources with optimizations"
      echo "Running extended test suite"
      echo "Generating documentation"
      echo "Build complete!"
    '';
  }
