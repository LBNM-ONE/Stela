//! `--gpu-check`'s first RADV commands: an instance through RADV's loader
//! entry, then a device on a compute queue and one on the universal queue,
//! each filling a buffer in host-visible memory twice, a small fill RADV
//! writes with CP DMA and a large one it writes with a compute shader ACO
//! compiles, each submitted with a fence and read back. Mesa and
//! ACO want a process's stack, so it runs on one of its own, after the image's
//! C and C++ constructors. Each step logs a line before it runs; after a
//! refusal nothing is torn down, as the boot stops once the check is done.
use super::super::{
    client, installed,
    stack::{self, Stack},
};
use crate::output;
use ash::{ext::debug_utils, vk};
use core::{
    ffi::{c_char, c_void},
    fmt,
};

/// A process's main stack, as the Metal service gives Mesa.
const STACK_BYTES: usize = 16 << 20;
/// What the fills write.
const FILL: u32 = 0xcafe_f00d;
/// The fills, by how RADV writes them: under 4 KiB with CP DMA, from 4 KiB
/// with a compute shader (`RADV_BUFFER_OPS_CS_THRESHOLD`).
const FILLS: [(&str, u64); 2] = [("cp dma", 256), ("shader", 1 << 20)];
/// How long a fill may take before the check calls it lost.
const WAIT_NS: u64 = 5_000_000_000;

/// RADV's commands, once the GPUs answered the DRM calls.
pub(super) fn radv() {
    let Some(gpu) = installed() else {
        return;
    };
    let Some(stack) = Stack::map(gpu.window, STACK_BYTES) else {
        say(format_args!("no memory for a stack"));
        return;
    };
    let top = stack.top();
    // Mesa's threads may hold its frames past the check, which ends the boot.
    core::mem::forget(stack);
    let mut body = || {
        say(format_args!("constructors"));
        // SAFETY: the one call, before any other of the C's.
        unsafe { stela_mesa::stela_constructors() };
        if let Err(step) = check() {
            say(format_args!("stopped at {step}"));
        }
    };
    // SAFETY: nothing else runs on the new stack.
    unsafe { stack::on_stack(top, &mut body) };
}

/// A `GPU CHECK radv` line.
fn say(what: fmt::Arguments) {
    output::arguments(format_args!("GPU CHECK radv: {what}\n"));
}

/// Logs `step`'s refusal with Vulkan's result, and names the step.
fn refused(step: &'static str) -> impl FnOnce(vk::Result) -> &'static str {
    move |error| {
        say(format_args!("{step} refused: {error:?}"));
        step
    }
}

/// RADV's loader entry, as Vulkan types it.
unsafe extern "system" fn entry(
    instance: vk::Instance,
    name: *const c_char,
) -> vk::PFN_vkVoidFunction {
    use vk::Handle;
    // SAFETY: the loader's call, and both ends are one function pointer.
    unsafe {
        core::mem::transmute(stela_mesa::vk_icdGetInstanceProcAddr(
            instance.as_raw() as *mut c_void,
            name,
        ))
    }
}

/// RADV's own messages, which its errors carry and Mesa prints only to a
/// messenger (`vk_log.c`), as `GPU CHECK radv: driver` lines.
unsafe extern "system" fn driver_message(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _kinds: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _context: *mut c_void,
) -> vk::Bool32 {
    // SAFETY: the driver's callback data, live for the call.
    let message = unsafe { data.as_ref().and_then(|data| data.message_as_c_str()) }
        .and_then(|message| message.to_str().ok())
        .unwrap_or("?");
    say(format_args!("driver {:#x}: {message}", severity.as_raw()));
    vk::FALSE
}

fn check() -> Result<(), &'static str> {
    let static_fn = ash::StaticFn {
        get_instance_proc_addr: entry,
    };
    // SAFETY: RADV's own entry, linked into the image.
    let entry = unsafe { ash::Entry::from_static_fn(static_fn) };
    let application = vk::ApplicationInfo::default()
        .application_name(c"Stela GPU check")
        .engine_name(c"Stela")
        .api_version(vk::API_VERSION_1_3);
    let names = [debug_utils::NAME.as_ptr()];
    let mut messages = vk::DebugUtilsMessengerCreateInfoEXT::default()
        .message_severity(
            vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
        )
        .message_type(
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
        )
        .pfn_user_callback(Some(driver_message));
    // The messenger in the chain hears the instance's creation; the one made
    // after it, everything until its destruction.
    let info = vk::InstanceCreateInfo::default()
        .application_info(&application)
        .enabled_extension_names(&names)
        .push_next(&mut messages);
    say(format_args!("instance"));
    // SAFETY: the create info and what it points to outlive the call.
    let instance = unsafe { entry.create_instance(&info, None) }.map_err(refused("instance"))?;
    let utils = debug_utils::Instance::new(&entry, &instance);
    // SAFETY: the instance enables the extension; `messages` chains nothing.
    let messenger = unsafe { utils.create_debug_utils_messenger(&messages, None) }
        .map_err(refused("messenger"))?;
    on_instance(&instance)?;
    say(format_args!("instance destroy"));
    // SAFETY: its one device is destroyed, and nothing else uses it.
    unsafe {
        utils.destroy_debug_utils_messenger(messenger, None);
        instance.destroy_instance(None);
    }
    say(format_args!("done"));
    Ok(())
}

fn on_instance(instance: &ash::Instance) -> Result<(), &'static str> {
    say(format_args!("physical devices"));
    // SAFETY: a live instance.
    let physicals = unsafe { instance.enumerate_physical_devices() }
        .map_err(refused("physical devices"))?;
    say(format_args!("{} physical devices", physicals.len()));
    let &physical = physicals.first().ok_or("physical devices")?;
    // SAFETY: a physical device of this instance.
    let properties = unsafe { instance.get_physical_device_properties(physical) };
    let name = properties
        .device_name_as_c_str()
        .ok()
        .and_then(|name| name.to_str().ok())
        .unwrap_or("?");
    say(format_args!(
        "{name}: vendor={:#06x} device={:#06x} vulkan={}.{}.{} driver={:#x}",
        properties.vendor_id,
        properties.device_id,
        vk::api_version_major(properties.api_version),
        vk::api_version_minor(properties.api_version),
        vk::api_version_patch(properties.api_version),
        properties.driver_version
    ));
    // SAFETY: as above.
    let families = unsafe { instance.get_physical_device_queue_family_properties(physical) };
    for (index, family) in families.iter().enumerate() {
        say(format_args!(
            "family {index}: flags={:#x} queues={}",
            family.queue_flags.as_raw(),
            family.queue_count
        ));
    }
    // A compute queue first: on GFX11 and later only the universal one needs
    // the attribute rings RADV makes at its first submission, so the two
    // tell a submission's failure from those rings'.
    let compute = families.iter().position(|family| {
        family.queue_flags.contains(vk::QueueFlags::COMPUTE)
            && !family.queue_flags.contains(vk::QueueFlags::GRAPHICS)
    });
    let universal = families.iter().position(|family| {
        family
            .queue_flags
            .contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE)
    });
    // SAFETY: as above.
    let memory = unsafe { instance.get_physical_device_memory_properties(physical) };
    let mut stopped = None;
    for (kind, family) in [("compute", compute), ("universal", universal)] {
        let Some(family) = family else {
            say(format_args!("no {kind} family"));
            continue;
        };
        if let Err(step) = on_family(instance, physical, &memory, kind, family as u32) {
            say(format_args!("{kind} stopped at {step}"));
            stopped = Some(step);
        }
    }
    stopped.map_or(Ok(()), Err)
}

/// A device with one queue of `family`, the fills on it, and its destruction;
/// a failed device is left as it is.
fn on_family(
    instance: &ash::Instance,
    physical: vk::PhysicalDevice,
    memory: &vk::PhysicalDeviceMemoryProperties,
    kind: &str,
    family: u32,
) -> Result<(), &'static str> {
    let priority = [1.0];
    let queues = [vk::DeviceQueueCreateInfo::default()
        .queue_family_index(family)
        .queue_priorities(&priority)];
    let info = vk::DeviceCreateInfo::default().queue_create_infos(&queues);
    say(format_args!("{kind}: device on family {family}"));
    // SAFETY: every create info outlives the call.
    let device =
        unsafe { instance.create_device(physical, &info, None) }.map_err(refused("device"))?;
    // SAFETY: the device's one queue of `family`.
    let queue = unsafe { device.get_device_queue(family, 0) };
    for (name, bytes) in FILLS {
        fill(&device, queue, family, memory, name, bytes)?;
    }
    say(format_args!("{kind}: device destroy"));
    // SAFETY: its objects are destroyed, and its queue is idle.
    unsafe { device.destroy_device(None) };
    Ok(())
}

/// A host-visible memory type `bits` allows, cached for the CPU's read back
/// where there is one.
fn host_visible(memory: &vk::PhysicalDeviceMemoryProperties, bits: u32) -> Option<u32> {
    let wanted = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
    let allowed = |index: u32| {
        bits & (1 << index) != 0
            && memory.memory_types[index as usize]
                .property_flags
                .contains(wanted)
    };
    let cached = |index: u32| {
        memory.memory_types[index as usize]
            .property_flags
            .contains(vk::MemoryPropertyFlags::HOST_CACHED)
    };
    let count = memory.memory_type_count;
    (0..count)
        .find(|&index| allowed(index) && cached(index))
        .or_else(|| (0..count).find(|&index| allowed(index)))
}

/// `bytes` of a buffer filled with [`FILL`] by the GPU and read back.
fn fill(
    device: &ash::Device,
    queue: vk::Queue,
    family: u32,
    memory: &vk::PhysicalDeviceMemoryProperties,
    name: &str,
    bytes: u64,
) -> Result<(), &'static str> {
    let info = vk::BufferCreateInfo::default()
        .size(bytes)
        .usage(vk::BufferUsageFlags::TRANSFER_DST)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    say(format_args!("{name}: buffer of {bytes} bytes"));
    // SAFETY: a live device, and the info outlives the call.
    let buffer = unsafe { device.create_buffer(&info, None) }.map_err(refused("buffer"))?;
    // SAFETY: the device's buffer.
    let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
    let kind = host_visible(memory, requirements.memory_type_bits).ok_or("host-visible memory")?;
    let allocate = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(kind);
    say(format_args!("{name}: memory of type {kind}"));
    // SAFETY: as above.
    let backing = unsafe { device.allocate_memory(&allocate, None) }.map_err(refused("memory"))?;
    // SAFETY: the buffer and its memory are the device's, and unbound.
    unsafe { device.bind_buffer_memory(buffer, backing, 0) }.map_err(refused("binding"))?;
    // SAFETY: host-visible memory, unmapped.
    let mapped = unsafe { device.map_memory(backing, 0, bytes, vk::MemoryMapFlags::empty()) }
        .map_err(refused("mapping"))?
        .cast::<u32>();
    let words = (bytes / 4) as usize;
    for at in 0..words {
        // SAFETY: the mapping holds `bytes` until it is unmapped below.
        unsafe { mapped.add(at).write_volatile(0) };
    }
    let pool_info = vk::CommandPoolCreateInfo::default().queue_family_index(family);
    // SAFETY: as above.
    let pool =
        unsafe { device.create_command_pool(&pool_info, None) }.map_err(refused("command pool"))?;
    let command_info = vk::CommandBufferAllocateInfo::default()
        .command_pool(pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(1);
    // SAFETY: the device's pool.
    let commands = unsafe { device.allocate_command_buffers(&command_info) }
        .map_err(refused("command buffer"))?;
    let command = commands[0];
    let begin =
        vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    let barrier = [vk::MemoryBarrier::default()
        .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
        .dst_access_mask(vk::AccessFlags::HOST_READ)];
    say(format_args!("{name}: record"));
    // SAFETY: a new command buffer of the device's, recording only its objects.
    unsafe {
        device
            .begin_command_buffer(command, &begin)
            .map_err(refused("recording"))?;
        device.cmd_fill_buffer(command, buffer, 0, bytes, FILL);
        device.cmd_pipeline_barrier(
            command,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::HOST,
            vk::DependencyFlags::empty(),
            &barrier,
            &[],
            &[],
        );
        device
            .end_command_buffer(command)
            .map_err(refused("recording"))?;
    }
    // SAFETY: as above.
    let fence = unsafe { device.create_fence(&vk::FenceCreateInfo::default(), None) }
        .map_err(refused("fence"))?;
    let submits = [vk::SubmitInfo::default().command_buffers(&commands)];
    say(format_args!("{name}: submit"));
    let started = client::now_ns();
    // SAFETY: a recorded command buffer of the queue's family, and an
    // unsignaled fence.
    unsafe { device.queue_submit(queue, &submits, fence) }.map_err(refused("submission"))?;
    say(format_args!("{name}: wait"));
    // SAFETY: the device's fence.
    unsafe { device.wait_for_fences(&[fence], true, WAIT_NS) }.map_err(refused("completion"))?;
    let took = client::now_ns().saturating_sub(started);
    let wrong = (0..words)
        // SAFETY: as above; the barrier made the fill visible to the host.
        .filter(|&at| unsafe { mapped.add(at).read_volatile() } != FILL)
        .count();
    say(format_args!(
        "{name}: done in {} us, {wrong} of {words} words wrong",
        took / 1000
    ));
    // SAFETY: the fill is complete, so nothing of these is in use.
    unsafe {
        device.destroy_fence(fence, None);
        device.free_command_buffers(pool, &commands);
        device.destroy_command_pool(pool, None);
        device.unmap_memory(backing);
        device.free_memory(backing, None);
        device.destroy_buffer(buffer, None);
    }
    if wrong == 0 {
        Ok(())
    } else {
        Err("read back")
    }
}
