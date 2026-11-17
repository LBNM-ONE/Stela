//! Links the archives `cargo xtask gpu nvk` or `cargo xtask gpu radv` builds.
use std::{env, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("set by Cargo"));
    let gpu = manifest.join("../../target/gpu");
    let nvk = env::var_os("CARGO_FEATURE_NVK").is_some();
    let radv = env::var_os("CARGO_FEATURE_RADV").is_some();
    assert!(
        nvk != radv,
        "stela-mesa links one Vulkan driver: enable `nvk` or `radv`"
    );
    let (lane, driver) = if nvk {
        ("nvk", "stela-nvk")
    } else {
        ("radv", "stela-radv")
    };
    // The lane's own directory first, as RADV's holds a `libstela-vulkan.a`
    // too; LLVM's archive is in NVK's.
    let directory = gpu.join(lane);
    let llvm = gpu.join("nvk");
    println!("cargo:rustc-link-search=native={}", directory.display());
    if radv {
        println!("cargo:rustc-link-search=native={}", llvm.display());
    }
    // Mesa links its Vulkan runtime and drivers whole: their entry points
    // meet in tables by weak reference, which pulls no member from an archive.
    for (directory, name, kind) in [
        (&directory, "stela-vulkan", "static:+whole-archive"),
        (&directory, driver, "static"),
        (&llvm, "stela-llvm", "static"),
    ] {
        let archive = directory.join(format!("lib{name}.a"));
        assert!(
            archive.is_file(),
            "{} is missing: run `cargo xtask gpu {lane}`",
            archive.display()
        );
        println!("cargo:rerun-if-changed={}", archive.display());
        println!("cargo:rustc-link-lib={kind}={name}");
    }
    if radv {
        // RADV keys no pipeline store yet: its archives stand for its compiler.
        stela_fingerprint::emit(
            "STELA_MESA_COMPILER_SHA256",
            &[
                ".",
                "../../target/gpu/radv/libstela-radv.a",
                "../../target/gpu/radv/libstela-vulkan.a",
                "../../rust-toolchain.toml",
            ],
        );
        return;
    }
    // What turns NIR into machine code: the C archives (NVK, NIR and the
    // Vulkan runtime) and the Rust NAK and NIL build from, with the
    // toolchain. NVK's own pipeline cache UUID is the same for every build
    // (`compat/mesa/build_id.c`), so this keys the pipeline store instead.
    stela_fingerprint::emit(
        "STELA_MESA_COMPILER_SHA256",
        &[
            ".",
            "../../target/gpu/nvk/libstela-nvk.a",
            "../../target/gpu/nvk/libstela-vulkan.a",
            "../../target/gpu/nvk/build/src/compiler/rust/libcompiler.rlib.p",
            "../../target/gpu/nvk/build/src/nouveau/compiler/latencies",
            "../../target/gpu/nvk/build/src/nouveau/compiler/nak_bindings.rs",
            "../../target/gpu/nvk/build/src/nouveau/headers/lib.rs",
            "../../target/gpu/nvk/build/src/nouveau/nil/nil_bindings.rs",
            "../../third_party/mesa/src/compiler/rust",
            "../../third_party/mesa/src/nouveau/compiler",
            "../../third_party/mesa/src/nouveau/headers/nv_push_rs",
            "../../third_party/mesa/src/nouveau/nil",
            "../../third_party/mesa/src/nouveau/rust",
            "../../rust-toolchain.toml",
        ],
    );
}
