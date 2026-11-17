//! RADV, ACO and NIR for `stela.efi`, by the NVK lane's steps.
//!
//! `cargo xtask boot build --radv` links them with the GPU client that
//! answers Mesa's calls into Stela (`stela_libc_*`); nothing starts RADV yet.
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
/// shaders that build them. libdrm_amdgpu, which Mesa's `ac_linux_drm.c` still
/// calls for devices, buffers and GPU addresses, is libdrm's own (MIT) over
/// `compat/mesa/drm.c`; with no `amdgpu.ids` in the image it names no GPU.
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
    stand_ins: &["drm", "cpu_model"],
    sources: &[
        "libdrm/amdgpu/amdgpu_asic_id.c",
        "libdrm/amdgpu/amdgpu_bo.c",
        "libdrm/amdgpu/amdgpu_cs.c",
        "libdrm/amdgpu/amdgpu_device.c",
        "libdrm/amdgpu/amdgpu_gpu_info.c",
        "libdrm/amdgpu/amdgpu_vamgr.c",
        "libdrm/amdgpu/handle_table.c",
    ],
    source_defines: &[
        "HAVE_LIBDRM_ATOMIC_PRIMITIVES=1",
        "AMDGPU_ASIC_ID_TABLE=\"/usr/share/libdrm/amdgpu.ids\"",
        // Names the image's own amdgpu, the kernel driver, also defines; Mesa
        // calls neither of libdrm's.
        "amdgpu_bo_list_create=libdrm_amdgpu_bo_list_create",
        "amdgpu_bo_set_metadata=libdrm_amdgpu_bo_set_metadata",
    ],
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
