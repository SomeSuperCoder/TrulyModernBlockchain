# Nix Build Configuration

When building this Rust project, always use the nix-shell command instead of direct cargo commands.

## Build Command

Instead of running `cargo build --all`, use:

```bash
nix-shell -p clang llvmPackages.libclang.lib --run "LIBCLANG_PATH=$(nix-build --no-out-link '<nixpkgs>' -A llvmPackages.libclang.lib)/lib cargo build --all" 2>&1
```

This ensures the proper LLVM/clang dependencies are available for the build process.
