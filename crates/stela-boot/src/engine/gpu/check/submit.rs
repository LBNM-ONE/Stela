//! `--gpu-check`'s command submission without RADV: on a GPU's render node, a
//! context, a buffer of commands Stela writes itself (`WRITE_DATA` of a word
//! into a second buffer, padded with a `NOP`), both mapped into the file's GPU
//! address space, submitted with `AMDGPU_CS`, waited for with
//! `AMDGPU_WAIT_CS`, and the word read back. It runs on the compute ring with
//! the commands in cached GTT, in GTT asked write-combined (which Stela's
//! Linux turns down, as it cannot keep such pages coherent), in the same
//! mapped past the GPU's L2 as RADV maps its command buffers, and in
//! CPU-visible VRAM, where RADV puts them when it counts the PCIe link fast
//! enough; then on the graphics ring. A refusal ends only its own submission,
//! whose buffers and context are then left as they are.
use super::{
    COMMAND, CPU_ACCESS_REQUIRED, GEM_CLOSE, GEM_CREATE, GEM_MMAP, GTT, GemCreate, GemRequest,
    READ_WRITE, VRAM, WRITE, client, ioctl, refused, say,
};
use core::{ffi::c_int, mem::size_of, ptr};

/// `DRM_IOCTL_AMDGPU_CTX`, of `union drm_amdgpu_ctx`: four words.
const CTX: u32 = ioctl(READ_WRITE, COMMAND + 0x02, size_of::<[u32; 4]>());
/// `DRM_IOCTL_AMDGPU_CS`.
const CS: u32 = ioctl(READ_WRITE, COMMAND + 0x04, size_of::<Cs>());
/// `DRM_IOCTL_AMDGPU_GEM_VA`, of this uapi's `struct drm_amdgpu_gem_va`.
const GEM_VA: u32 = ioctl(WRITE, COMMAND + 0x08, size_of::<GemVa>());
/// `DRM_IOCTL_AMDGPU_WAIT_CS`.
const WAIT_CS: u32 = ioctl(READ_WRITE, COMMAND + 0x09, size_of::<WaitCs>());
const _: () = assert!(CTX == 0xc010_6442 && CS == 0xc018_6444);
const _: () = assert!(GEM_VA == 0x4040_6448 && WAIT_CS == 0xc020_6449);

/// `AMDGPU_CTX_OP_ALLOC_CTX` and `AMDGPU_CTX_OP_FREE_CTX`.
const ALLOC_CTX: u32 = 1;
const FREE_CTX: u32 = 2;
/// `AMDGPU_CHUNK_ID_IB` and `AMDGPU_CHUNK_ID_BO_HANDLES`.
const CHUNK_IB: u32 = 1;
const CHUNK_BO_HANDLES: u32 = 6;
/// `AMDGPU_VA_OP_MAP`, with `AMDGPU_VM_PAGE_READABLE`, `WRITEABLE` and
/// `EXECUTABLE`.
const VA_MAP: u32 = 1;
const VA_ACCESS: u32 = 0x2 | 0x4 | 0x8;
/// `AMDGPU_GEM_CREATE_CPU_GTT_USWC`: GTT the CPU writes combined and the GPU
/// reads unsnooped, as RADV asks for its command buffers.
const USWC: u64 = 0x4;
/// `AMDGPU_VM_MTYPE_UC`: the GPU's L2 bypassed, as RADV maps its command
/// buffers (`RADEON_FLAG_GL2_BYPASS`).
const UNCACHED: u32 = 4 << 5;
/// `AMDGPU_HW_IP_GFX` and `AMDGPU_HW_IP_COMPUTE`.
const GFX: u32 = 0;
const COMPUTE: u32 = 1;
/// What `WRITE_DATA` writes: "STEL".
const WORD: u32 = u32::from_le_bytes(*b"STEL");
/// The buffers' size, and the GPU address of the first pair; each submission
/// takes the next 2 MiB of the file's address space.
const PAGE: usize = 4096;
const FIRST_ADDRESS: u64 = 1 << 32;
/// How long a submission may take before the check calls it lost.
const WAIT_NS: u64 = 2_000_000_000;

/// The submissions, by ring and by where their commands lie.
const SUBMISSIONS: [(&str, u32, Placement); 5] = [
    ("compute cached", COMPUTE, CACHED),
    (
        "compute write-combined",
        COMPUTE,
        Placement {
            domains: GTT,
            flags: USWC,
            mapping: 0,
        },
    ),
    (
        "compute write-combined past l2",
        COMPUTE,
        Placement {
            domains: GTT,
            flags: USWC,
            mapping: UNCACHED,
        },
    ),
    (
        "compute vram past l2",
        COMPUTE,
        Placement {
            domains: VRAM,
            flags: CPU_ACCESS_REQUIRED,
            mapping: UNCACHED,
        },
    ),
    ("gfx cached", GFX, CACHED),
];

/// `struct drm_amdgpu_gem_va`, with the timeline fields this uapi added.
#[repr(C)]
#[derive(Default)]
#[allow(dead_code, reason = "the kernel's layout, which it reads")]
struct GemVa {
    handle: u32,
    pad: u32,
    operation: u32,
    flags: u32,
    va_address: u64,
    offset_in_bo: u64,
    map_size: u64,
    vm_timeline_point: u64,
    vm_timeline_syncobj_out: u32,
    num_syncobj_handles: u32,
    input_fence_syncobj_handles: u64,
}

/// `struct drm_amdgpu_cs_chunk`.
#[repr(C)]
#[allow(dead_code, reason = "the kernel's layout, which it reads")]
struct Chunk {
    id: u32,
    length_dw: u32,
    data: u64,
}

/// `struct drm_amdgpu_cs_chunk_ib`.
#[repr(C)]
#[allow(dead_code, reason = "the kernel's layout, which it reads")]
struct ChunkIb {
    pad: u32,
    flags: u32,
    va_start: u64,
    ib_bytes: u32,
    ip_type: u32,
    ip_instance: u32,
    ring: u32,
}

/// `struct drm_amdgpu_bo_list_in`.
#[repr(C)]
#[allow(dead_code, reason = "the kernel's layout, which it reads")]
struct BoList {
    operation: u32,
    list_handle: u32,
    bo_number: u32,
    bo_info_size: u32,
    bo_info_ptr: u64,
}

/// `union drm_amdgpu_cs`'s `in`; its `out` is the submission's sequence
/// number, over the first two words.
#[repr(C)]
#[allow(dead_code, reason = "the kernel's layout, which it reads")]
struct Cs {
    ctx_id: u32,
    bo_list_handle: u32,
    num_chunks: u32,
    flags: u32,
    chunks: u64,
}

/// `union drm_amdgpu_wait_cs`'s `in`; its `out` is whether the wait timed
/// out, over the first two words.
#[repr(C)]
#[allow(dead_code, reason = "the kernel's layout, which it reads")]
struct WaitCs {
    handle: u64,
    timeout: u64,
    ip_type: u32,
    ip_instance: u32,
    ring: u32,
    ctx_id: u32,
}

const _: () = assert!(size_of::<GemVa>() == 0x40 && size_of::<Chunk>() == 0x10);
const _: () = assert!(size_of::<ChunkIb>() == 0x20 && size_of::<BoList>() == 0x18);
const _: () = assert!(size_of::<Cs>() == 0x18 && size_of::<WaitCs>() == 0x20);

/// Each submission in turn, each logged.
pub(super) fn submissions(fd: c_int, minor: u32) {
    for (index, (name, ring, placement)) in SUBMISSIONS.into_iter().enumerate() {
        let address = FIRST_ADDRESS + index as u64 * (2 << 20);
        if let Err(step) = submission(fd, minor, name, ring, placement, address) {
            say(minor, format_args!("{name}: stopped at {step}"));
        }
    }
}

/// The commands: `WRITE_DATA` of [`WORD`] to memory at `target`, its write
/// confirmed, then a `NOP` to the 8 dwords both rings align a buffer to.
fn commands(target: u64) -> [u32; 8] {
    /// `PKT3(PKT3_WRITE_DATA, 3, 0)`.
    const WRITE_DATA: u32 = (3 << 30) | (3 << 16) | (0x37 << 8);
    /// `WRITE_DATA_DST_SEL(5)`, memory, and `WRITE_DATA_WR_CONFIRM`.
    const TO_MEMORY: u32 = (5 << 8) | (1 << 20);
    /// `PKT3(PKT3_NOP, 1, 0)`: the header and two words.
    const NOP: u32 = (3 << 30) | (1 << 16) | (0x10 << 8);
    [
        WRITE_DATA,
        TO_MEMORY,
        target as u32,
        (target >> 32) as u32,
        WORD,
        NOP,
        0,
        0,
    ]
}

/// Where a buffer lies, and how the GPU maps it.
#[derive(Clone, Copy)]
struct Placement {
    domains: u64,
    flags: u64,
    /// `AMDGPU_VM_MTYPE_*`, or 0 for the kernel's default.
    mapping: u32,
}

/// Cached GTT, as the kernel maps it by default.
const CACHED: Placement = Placement {
    domains: GTT,
    flags: 0,
    mapping: 0,
};

/// A page placed as `placement` says, mapped at `address` in the file's GPU
/// address space: its handle.
fn buffer(fd: c_int, minor: u32, placement: Placement, address: u64) -> Result<u32, &'static str> {
    let mut create = GemCreate {
        request: GemRequest {
            size: PAGE as u64,
            alignment: PAGE as u64,
            domains: placement.domains,
            flags: placement.flags,
        },
    };
    refused(
        minor,
        client::control(fd, GEM_CREATE, (&raw mut create).cast()),
        "buffer",
    )?;
    // SAFETY: on success the kernel wrote the handle over the request.
    let handle = unsafe { create.handle };
    let mut map = GemVa {
        handle,
        operation: VA_MAP,
        flags: VA_ACCESS | placement.mapping,
        va_address: address,
        map_size: PAGE as u64,
        ..GemVa::default()
    };
    refused(
        minor,
        client::control(fd, GEM_VA, (&raw mut map).cast()),
        "gpu address",
    )?;
    Ok(handle)
}

/// The CPU's mapping of `handle`.
fn cpu_map(fd: c_int, minor: u32, handle: u32) -> Result<*mut u32, &'static str> {
    let mut offset = u64::from(handle);
    refused(
        minor,
        client::control(fd, GEM_MMAP, (&raw mut offset).cast()),
        "offset",
    )?;
    let address = client::map(fd, offset, PAGE).ok_or("cpu mapping")?;
    Ok(address.as_ptr().cast())
}

fn submission(
    fd: c_int,
    minor: u32,
    name: &str,
    ring: u32,
    commands_placement: Placement,
    address: u64,
) -> Result<(), &'static str> {
    let target_address = address + (1 << 20);
    say(minor, format_args!("{name}: context"));
    let mut context = [ALLOC_CTX, 0, 0, 0];
    refused(
        minor,
        client::control(fd, CTX, (&raw mut context).cast()),
        "context",
    )?;
    // `out.alloc.ctx_id`, over the request's first word.
    let context_id = context[0];
    say(minor, format_args!("{name}: buffers"));
    let commands_handle = buffer(fd, minor, commands_placement, address)?;
    let target_handle = buffer(fd, minor, CACHED, target_address)?;
    let written = cpu_map(fd, minor, commands_handle)?;
    for (at, word) in commands(target_address).into_iter().enumerate() {
        // SAFETY: the mapping holds a page until it is unmapped below.
        unsafe { written.add(at).write_volatile(word) };
    }
    // SAFETY: the mapping `cpu_map` made.
    client::unmap(unsafe { ptr::NonNull::new_unchecked(written.cast()) });
    let target = cpu_map(fd, minor, target_handle)?;
    // SAFETY: as above.
    unsafe { target.write_volatile(0) };
    let ib = ChunkIb {
        pad: 0,
        flags: 0,
        va_start: address,
        ib_bytes: (commands(0).len() * size_of::<u32>()) as u32,
        ip_type: ring,
        ip_instance: 0,
        ring: 0,
    };
    let entries = [[commands_handle, 0], [target_handle, 0]];
    let list = BoList {
        operation: !0,
        list_handle: !0,
        bo_number: entries.len() as u32,
        bo_info_size: size_of::<[u32; 2]>() as u32,
        bo_info_ptr: entries.as_ptr() as u64,
    };
    let chunks = [
        Chunk {
            id: CHUNK_IB,
            length_dw: (size_of::<ChunkIb>() / 4) as u32,
            data: ptr::from_ref(&ib) as u64,
        },
        Chunk {
            id: CHUNK_BO_HANDLES,
            length_dw: (size_of::<BoList>() / 4) as u32,
            data: ptr::from_ref(&list) as u64,
        },
    ];
    let pointers = [
        ptr::from_ref(&chunks[0]) as u64,
        ptr::from_ref(&chunks[1]) as u64,
    ];
    let mut submit = Cs {
        ctx_id: context_id,
        bo_list_handle: 0,
        num_chunks: pointers.len() as u32,
        flags: 0,
        chunks: pointers.as_ptr() as u64,
    };
    say(minor, format_args!("{name}: submit"));
    let started = client::now_ns();
    refused(
        minor,
        client::control(fd, CS, (&raw mut submit).cast()),
        "submission",
    )?;
    // SAFETY: on success the kernel wrote the sequence number over the
    // request's first two words.
    let sequence = unsafe { (&raw const submit).cast::<u64>().read() };
    let mut wait = WaitCs {
        handle: sequence,
        timeout: client::now_ns() + WAIT_NS,
        ip_type: ring,
        ip_instance: 0,
        ring: 0,
        ctx_id: context_id,
    };
    say(minor, format_args!("{name}: wait for {sequence}"));
    refused(
        minor,
        client::control(fd, WAIT_CS, (&raw mut wait).cast()),
        "wait",
    )?;
    let took = client::now_ns().saturating_sub(started);
    // SAFETY: the kernel wrote `out.status` over the request's first words.
    let timed_out = unsafe { (&raw const wait).cast::<u64>().read() };
    // SAFETY: as above.
    let word = unsafe { target.read_volatile() };
    say(
        minor,
        format_args!(
            "{name}: timed_out={timed_out} word={word:#x} in {} us{}",
            took / 1000,
            if word == WORD { "" } else { ", wrong" }
        ),
    );
    // SAFETY: as above.
    client::unmap(unsafe { ptr::NonNull::new_unchecked(target.cast()) });
    for handle in [commands_handle, target_handle] {
        let mut close = [handle, 0];
        client::control(fd, GEM_CLOSE, (&raw mut close).cast());
    }
    let mut free = [FREE_CTX, 0, context_id, 0];
    client::control(fd, CTX, (&raw mut free).cast());
    if word == WORD {
        Ok(())
    } else {
        Err("read back")
    }
}
