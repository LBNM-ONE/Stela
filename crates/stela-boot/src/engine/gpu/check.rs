//! `--gpu-check`'s look at the DRM calls RADV makes through libdrm, on each
//! GPU amdgpu bound: the version and `AMDGPU_INFO` queries a device's opening
//! asks (`amdgpu_device_initialize`), then a buffer in GTT and one in
//! CPU-visible VRAM created, mapped, written and read back, as
//! `amdgpu_bo_alloc` and `amdgpu_bo_cpu_map` do. Each step logs a line before
//! it runs, so a stall shows where it stopped, and a refusal ends that GPU's
//! check, as the steps after it need it. Then commands Stela submits itself
//! ([`submit`]), and when every GPU answered, RADV's own ([`vulkan`]).
mod submit;
mod vulkan;

use super::{client, installed};
use crate::output;
use core::{
    ffi::c_int,
    fmt,
    mem::{offset_of, size_of},
    ptr,
};

/// Linux's `_IOC` for DRM's ioctls, of type `'d'`, on x86-64.
const fn ioctl(direction: u32, number: u32, size: usize) -> u32 {
    (direction << 30) | ((size as u32) << 16) | ((b'd' as u32) << 8) | number
}
const WRITE: u32 = 1;
const READ_WRITE: u32 = 3;
/// `DRM_COMMAND_BASE`: where a driver's own ioctls start.
const COMMAND: u32 = 0x40;
/// `DRM_IOCTL_VERSION`.
const VERSION: u32 = ioctl(READ_WRITE, 0x00, size_of::<Version>());
/// `DRM_IOCTL_GEM_CLOSE`, of `struct drm_gem_close`: a handle and a pad.
const GEM_CLOSE: u32 = ioctl(WRITE, 0x09, size_of::<[u32; 2]>());
/// `DRM_IOCTL_AMDGPU_GEM_CREATE`.
const GEM_CREATE: u32 = ioctl(READ_WRITE, COMMAND, size_of::<GemCreate>());
/// `DRM_IOCTL_AMDGPU_GEM_MMAP`, of `union drm_amdgpu_gem_mmap`: the handle in,
/// the offset to map out, in one word.
const GEM_MMAP: u32 = ioctl(READ_WRITE, COMMAND + 0x01, size_of::<u64>());
/// `DRM_IOCTL_AMDGPU_INFO`.
const INFO: u32 = ioctl(WRITE, COMMAND + 0x05, size_of::<Info>());
const _: () = assert!(VERSION == 0xc040_6400 && GEM_CLOSE == 0x4008_6409);
const _: () = assert!(GEM_CREATE == 0xc020_6440 && GEM_MMAP == 0xc008_6441);
const _: () = assert!(INFO == 0x4020_6445);

/// `AMDGPU_INFO_*`: what `amdgpu_device_initialize` and RADV's device
/// queries ask first.
const ACCEL_WORKING: u32 = 0x00;
const HW_IP_INFO: u32 = 0x02;
const HW_IP_COUNT: u32 = 0x03;
const VRAM_GTT: u32 = 0x14;
const DEV_INFO: u32 = 0x16;
/// `AMDGPU_HW_IP_*`: the engines RADV queues on.
const ENGINES: [(&str, u32); 3] = [("gfx", 0), ("compute", 1), ("sdma", 2)];
/// `AMDGPU_GEM_DOMAIN_GTT`, `AMDGPU_GEM_DOMAIN_VRAM` and
/// `AMDGPU_GEM_CREATE_CPU_ACCESS_REQUIRED`.
const GTT: u64 = 0x2;
const VRAM: u64 = 0x4;
const CPU_ACCESS_REQUIRED: u64 = 1 << 0;
/// What each buffer holds.
const BUFFER_BYTES: usize = 64 * 1024;

/// `struct drm_version`.
#[repr(C)]
#[allow(dead_code, reason = "the kernel's layout, which it fills")]
struct Version {
    major: c_int,
    minor: c_int,
    patchlevel: c_int,
    name_length: usize,
    name: *mut u8,
    date_length: usize,
    date: *mut u8,
    description_length: usize,
    description: *mut u8,
}

/// `struct drm_amdgpu_info`, with `query_hw_ip`'s arguments.
#[repr(C)]
#[allow(dead_code, reason = "the kernel's layout, which it reads")]
struct Info {
    return_pointer: u64,
    return_size: u32,
    query: u32,
    arguments: [u32; 4],
}

/// `struct drm_amdgpu_info_hw_ip`.
#[repr(C)]
#[derive(Default)]
struct HwIp {
    version_major: u32,
    version_minor: u32,
    capabilities: u64,
    ib_start_alignment: u32,
    ib_size_alignment: u32,
    available_rings: u32,
    ip_discovery_version: u32,
}

/// `struct drm_amdgpu_info_vram_gtt`.
#[repr(C)]
#[derive(Default)]
struct VramGtt {
    vram: u64,
    vram_cpu_accessible: u64,
    gtt: u64,
}

/// `struct drm_amdgpu_info_device` through `high_va_max`, what libdrm's
/// device opening reads: the kernel writes no more than is asked.
#[repr(C)]
#[derive(Default)]
#[allow(dead_code, reason = "the kernel's layout, of which the check logs a few fields")]
struct DeviceInfo {
    device_id: u32,
    chip_rev: u32,
    external_rev: u32,
    pci_rev: u32,
    family: u32,
    num_shader_engines: u32,
    num_shader_arrays_per_engine: u32,
    gpu_counter_freq: u32,
    max_engine_clock: u64,
    max_memory_clock: u64,
    cu_active_number: u32,
    cu_ao_mask: u32,
    cu_bitmap: [[u32; 4]; 4],
    enabled_rb_pipes_mask: u32,
    num_rb_pipes: u32,
    num_hw_gfx_contexts: u32,
    pcie_gen: u32,
    ids_flags: u64,
    virtual_address_offset: u64,
    virtual_address_max: u64,
    virtual_address_alignment: u32,
    pte_fragment_size: u32,
    gart_page_size: u32,
    ce_ram_size: u32,
    vram_type: u32,
    vram_bit_width: u32,
    vce_harvest_config: u32,
    gc_double_offchip_lds_buf: u32,
    prim_buf_gpu_addr: u64,
    pos_buf_gpu_addr: u64,
    cntl_sb_buf_gpu_addr: u64,
    param_buf_gpu_addr: u64,
    prim_buf_size: u32,
    pos_buf_size: u32,
    cntl_sb_buf_size: u32,
    param_buf_size: u32,
    wave_front_size: u32,
    num_shader_visible_vgprs: u32,
    num_cu_per_sh: u32,
    num_tcc_blocks: u32,
    gs_vgt_table_depth: u32,
    gs_prim_buffer_depth: u32,
    max_gs_waves_per_vgt: u32,
    pcie_num_lanes: u32,
    cu_ao_bitmap: [[u32; 4]; 4],
    high_va_offset: u64,
    high_va_max: u64,
}
const _: () = assert!(offset_of!(DeviceInfo, virtual_address_offset) == 0x90);
const _: () = assert!(size_of::<DeviceInfo>() == 0x160);

/// `union drm_amdgpu_gem_create`.
#[repr(C)]
#[allow(dead_code, reason = "the kernel's layout, which it reads")]
union GemCreate {
    request: GemRequest,
    /// `out.handle`, which the kernel writes over the request.
    handle: u32,
}

/// `union drm_amdgpu_gem_create`'s `in`.
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code, reason = "the kernel's layout, which it reads")]
struct GemRequest {
    size: u64,
    alignment: u64,
    domains: u64,
    flags: u64,
}

/// Checks each GPU amdgpu bound, through its render node, then RADV on them
/// when all answered.
pub(in crate::engine) fn amdgpu() {
    let count = installed().map_or(0, |gpu| gpu.start.bound.len());
    let mut answered = count != 0;
    for index in 0..count as u32 {
        let minor = client::RENDER_MINOR + index;
        say(minor, format_args!("opening"));
        match client::open(minor) {
            Ok(fd) => {
                if let Err(step) = check(fd, minor) {
                    say(minor, format_args!("stopped at {step}"));
                    answered = false;
                }
                client::close_descriptor(fd);
            }
            Err(error) => {
                say(minor, format_args!("open refused: {error}"));
                answered = false;
            }
        }
    }
    if answered {
        vulkan::radv();
    }
}

/// A `GPU CHECK amdgpu` line for the node at `minor`.
fn say(minor: u32, what: fmt::Arguments) {
    output::arguments(format_args!("GPU CHECK amdgpu {minor}: {what}\n"));
}

fn check(fd: c_int, minor: u32) -> Result<(), &'static str> {
    let mut name = [0u8; 16];
    let mut version = Version {
        major: 0,
        minor: 0,
        patchlevel: 0,
        name_length: name.len(),
        name: name.as_mut_ptr(),
        date_length: 0,
        date: ptr::null_mut(),
        description_length: 0,
        description: ptr::null_mut(),
    };
    say(minor, format_args!("version"));
    refused(minor, client::control(fd, VERSION, (&raw mut version).cast()), "version")?;
    let named = &name[..version.name_length.min(name.len())];
    say(
        minor,
        format_args!(
            "driver {} {}.{}.{}",
            core::str::from_utf8(named).unwrap_or("?"),
            version.major,
            version.minor,
            version.patchlevel
        ),
    );

    let mut working = 0u32;
    say(minor, format_args!("query accel_working"));
    refused(minor, query(fd, ACCEL_WORKING, [0; 4], &mut working), "accel_working")?;
    say(minor, format_args!("accel_working={working}"));

    let mut device = DeviceInfo::default();
    say(minor, format_args!("query dev_info"));
    refused(minor, query(fd, DEV_INFO, [0; 4], &mut device), "dev_info")?;
    say(
        minor,
        format_args!(
            "device={:#06x} rev={:#04x} external_rev={:#04x} family={} shader_engines={} \
             active_cus={} wave={} vram_type={} vram_bits={} va={:#x}..{:#x} \
             high_va={:#x}..{:#x} va_alignment={:#x} gart_page={:#x}",
            device.device_id,
            device.pci_rev,
            device.external_rev,
            device.family,
            device.num_shader_engines,
            device.cu_active_number,
            device.wave_front_size,
            device.vram_type,
            device.vram_bit_width,
            device.virtual_address_offset,
            device.virtual_address_max,
            device.high_va_offset,
            device.high_va_max,
            device.virtual_address_alignment,
            device.gart_page_size
        ),
    );

    let mut sizes = VramGtt::default();
    say(minor, format_args!("query vram_gtt"));
    refused(minor, query(fd, VRAM_GTT, [0; 4], &mut sizes), "vram_gtt")?;
    say(
        minor,
        format_args!(
            "vram={} MiB cpu_visible={} MiB gtt={} MiB",
            sizes.vram >> 20,
            sizes.vram_cpu_accessible >> 20,
            sizes.gtt >> 20
        ),
    );

    for (engine, kind) in ENGINES {
        let mut count = 0u32;
        say(minor, format_args!("query {engine} count"));
        refused(minor, query(fd, HW_IP_COUNT, [kind, 0, 0, 0], &mut count), "hw_ip_count")?;
        let mut ip = HwIp::default();
        if count != 0 {
            refused(minor, query(fd, HW_IP_INFO, [kind, 0, 0, 0], &mut ip), "hw_ip_info")?;
        }
        say(
            minor,
            format_args!(
                "{engine}: instances={count} version={}.{} rings={:#x} ib_alignment={}/{} \
                 discovery={:#x} capabilities={:#x}",
                ip.version_major,
                ip.version_minor,
                ip.available_rings,
                ip.ib_start_alignment,
                ip.ib_size_alignment,
                ip.ip_discovery_version,
                ip.capabilities
            ),
        );
    }

    buffer(fd, minor, "gtt", GTT, 0)?;
    buffer(fd, minor, "vram", VRAM, CPU_ACCESS_REQUIRED)?;
    submit::submissions(fd, minor);
    Ok(())
}

/// `AMDGPU_INFO`'s `query` into `answer`, with `arguments` for the queries
/// that take any: zero or a negative errno.
fn query<T>(fd: c_int, query: u32, arguments: [u32; 4], answer: &mut T) -> i64 {
    let mut info = Info {
        return_pointer: ptr::from_mut(answer) as u64,
        return_size: size_of::<T>() as u32,
        query,
        arguments,
    };
    client::control(fd, INFO, (&raw mut info).cast())
}

/// `Err(step)`, logged, when `result` is a refusal.
fn refused(minor: u32, result: i64, step: &'static str) -> Result<(), &'static str> {
    if result == 0 {
        return Ok(());
    }
    say(minor, format_args!("{step} refused: {result}"));
    Err(step)
}

/// A buffer of `domains` created, mapped, written through the mapping and
/// read back, as RADV's first upload is; closed after.
fn buffer(fd: c_int, minor: u32, name: &str, domains: u64, flags: u64) -> Result<(), &'static str> {
    let mut create = GemCreate {
        request: GemRequest {
            size: BUFFER_BYTES as u64,
            alignment: 4096,
            domains,
            flags,
        },
    };
    say(minor, format_args!("{name} buffer create"));
    refused(minor, client::control(fd, GEM_CREATE, (&raw mut create).cast()), "buffer creation")?;
    // SAFETY: on success the kernel wrote the handle over the request.
    let handle = unsafe { create.handle };
    let mut mapped = u64::from(handle);
    say(minor, format_args!("{name} buffer {handle} offset"));
    let result = refused(minor, client::control(fd, GEM_MMAP, (&raw mut mapped).cast()), "buffer offset")
        .and_then(|()| {
            say(minor, format_args!("{name} buffer {handle} map at {mapped:#x}"));
            client::map(fd, mapped, BUFFER_BYTES).ok_or("buffer mapping")
        })
        .map(|address| {
            let words = address.as_ptr().cast::<u32>();
            let count = BUFFER_BYTES / size_of::<u32>();
            say(minor, format_args!("{name} buffer {handle} write at {address:p}"));
            for at in 0..count {
                // SAFETY: the mapping holds `BUFFER_BYTES` until it is unmapped below.
                unsafe { words.add(at).write_volatile(pattern(at)) };
            }
            say(minor, format_args!("{name} buffer {handle} read back"));
            let wrong = (0..count)
                // SAFETY: as above.
                .filter(|&at| unsafe { words.add(at).read_volatile() } != pattern(at))
                .count();
            client::unmap(address);
            say(minor, format_args!("{name} buffer {handle}: {wrong} of {count} words wrong"));
            wrong
        });
    let mut close = [handle, 0];
    let closed = client::control(fd, GEM_CLOSE, (&raw mut close).cast());
    if closed != 0 {
        say(minor, format_args!("{name} buffer {handle} close refused: {closed}"));
    }
    match result {
        Ok(0) => Ok(()),
        Ok(_) => Err("buffer read back"),
        Err(step) => Err(step),
    }
}

/// What the buffers hold at word `at`: no two neighbours alike.
fn pattern(at: usize) -> u32 {
    (at as u32).wrapping_mul(0x9e37_79b9) ^ 0xa5a5_5a5a
}
