//! RADV, ACO and NIR for `stela.efi`, by the NVK lane's steps.
//!
//! The image does not link these archives yet: Mesa's calls into Stela
//! (`stela_libc_*`) are answered by the GPU client only NVK's images build,
//! and libdrm_amdgpu's, in `compat/mesa/amdgpu.c`, still refuse with ENOTSUP.
use super::nvk::{self, Lane};
use anyhow::Result;

/// NVK's options, for AMD's driver: NVK's are part of the GPU builds'
/// identity, which then covers these as well.
pub(super) const OPTIONS: [&str; 21] = [
    "-Dcpp_rtti=false",
    "-Dvulkan-drivers=amd",
    "-Dgallium-drivers=",
    "-Dplatforms=",
    "-Dopengl=false",
    "-Dgles1=disabled",
    "-Dgles2=disabled",
    "-Dglx=disabled",
    "-Degl=disabled",
    "-Dgbm=disabled",
    "-Dllvm=disabled",
    "-Dzlib=disabled",
    "-Dzstd=disabled",
    "-Dshader-cache=disabled",
    "-Dmesa-clc=system",
    "-Dprecomp-compiler=system",
    "-Dvulkan-layers=",
    "-Dtools=",
    "-Dvideo-codecs=",
    "-Dexpat=disabled",
    "-Dbuildtype=release",
];

/// RADV links whole what NVK does, Vulkan's runtime, and its own units in
/// place of `libnvk.a`. Meson finds a libelf, the vendored `.pc` file or the
/// build host's, but the image has none: RADV's units build as without one,
/// and its ELF units stay out, which only LLVM's binaries need. Its device
/// enables acceleration structures, so the host's glslang compiles the
/// shaders that build them.
const RADV: Lane = Lane {
    name: "radv",
    options: &OPTIONS,
    drivers: &["src/amd/vulkan/libvulkan_radeon.so"],
    model_unit: "radv_query.c",
    whole_libraries: &[
        "libvulkan_lite_runtime_a",
        "libvulkan_runtime_a",
        "libvulkan_instance_a",
        "libvulkan_radeon_so",
    ],
    skipped: &[
        "ac_rtld",
        "ac_rgp_elf_object_pack",
        "build_id",
        "u_qsort",
        "os_file_notify",
        "os_socket",
        "disk_cache",
        "mesa_cache_db",
        "u_debug_stack",
        "wsi_common_display",
    ],
    stand_ins: &["drm", "amdgpu", "cpu_model"],
    ungenerated: &[],
    libdrm: &["amdgpu"],
    dropped_defines: &["USE_LIBELF"],
    host_glslang: true,
};

pub(super) fn build(jobs: u16) -> Result<()> {
    let configured = nvk::configure(&RADV, jobs)?;
    nvk::assemble(&RADV, &configured, jobs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_options_are_nvks_but_for_the_driver() {
        assert_eq!(OPTIONS.len(), nvk::OPTIONS.len());
        for (radv, nvk) in OPTIONS.iter().zip(nvk::OPTIONS) {
            if nvk.starts_with("-Dvulkan-drivers=") {
                assert_eq!(*radv, "-Dvulkan-drivers=amd");
            } else {
                assert_eq!(*radv, nvk);
            }
        }
    }
}
