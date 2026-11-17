// SPDX-License-Identifier: GPL-2.0-or-later
/*
 * The libdrm_amdgpu calls Mesa's ac_linux_drm.c still makes; the rest of its
 * calls go straight to the DRM ioctls drm.c carries. Until these are written
 * over amdgpu's ioctls in the Linux environment, each refuses, so RADV's
 * winsys does not start and it finds no device. Stela's own hooks, the
 * stela_libc_* functions, come from the image and never from here.
 */
#include <amdgpu.h>
#include <errno.h>
#include <stddef.h>

int amdgpu_device_initialize(int fd, uint32_t *major_version,
			     uint32_t *minor_version,
			     amdgpu_device_handle *device_handle)
{
	(void)fd;
	(void)major_version;
	(void)minor_version;
	(void)device_handle;
	return -ENOTSUP;
}

int amdgpu_device_deinitialize(amdgpu_device_handle device_handle)
{
	(void)device_handle;
	return -ENOTSUP;
}

int amdgpu_device_get_fd(amdgpu_device_handle device_handle)
{
	(void)device_handle;
	return -ENOTSUP;
}

int amdgpu_bo_alloc(amdgpu_device_handle dev,
		    struct amdgpu_bo_alloc_request *alloc_buffer,
		    amdgpu_bo_handle *buf_handle)
{
	(void)dev;
	(void)alloc_buffer;
	(void)buf_handle;
	return -ENOTSUP;
}

int amdgpu_bo_export(amdgpu_bo_handle buf_handle,
		     enum amdgpu_bo_handle_type type, uint32_t *shared_handle)
{
	(void)buf_handle;
	(void)type;
	(void)shared_handle;
	return -ENOTSUP;
}

int amdgpu_bo_import(amdgpu_device_handle dev,
		     enum amdgpu_bo_handle_type type, uint32_t shared_handle,
		     struct amdgpu_bo_import_result *output)
{
	(void)dev;
	(void)type;
	(void)shared_handle;
	(void)output;
	return -ENOTSUP;
}

int amdgpu_create_bo_from_user_mem(amdgpu_device_handle dev, void *cpu,
				   uint64_t size, amdgpu_bo_handle *buf_handle)
{
	(void)dev;
	(void)cpu;
	(void)size;
	(void)buf_handle;
	return -ENOTSUP;
}

int amdgpu_bo_free(amdgpu_bo_handle buf_handle)
{
	(void)buf_handle;
	return -ENOTSUP;
}

int amdgpu_bo_cpu_map(amdgpu_bo_handle buf_handle, void **cpu)
{
	(void)buf_handle;
	(void)cpu;
	return -ENOTSUP;
}

int amdgpu_bo_cpu_unmap(amdgpu_bo_handle buf_handle)
{
	(void)buf_handle;
	return -ENOTSUP;
}

int amdgpu_query_sw_info(amdgpu_device_handle dev, enum amdgpu_sw_info info,
			 void *value)
{
	(void)dev;
	(void)info;
	(void)value;
	return -ENOTSUP;
}

int amdgpu_va_range_alloc(amdgpu_device_handle dev,
			  enum amdgpu_gpu_va_range va_range_type,
			  uint64_t size, uint64_t va_base_alignment,
			  uint64_t va_base_required, uint64_t *va_base_allocated,
			  amdgpu_va_handle *va_range_handle, uint64_t flags)
{
	(void)dev;
	(void)va_range_type;
	(void)size;
	(void)va_base_alignment;
	(void)va_base_required;
	(void)va_base_allocated;
	(void)va_range_handle;
	(void)flags;
	return -ENOTSUP;
}

int amdgpu_va_range_free(amdgpu_va_handle va_range_handle)
{
	(void)va_range_handle;
	return -ENOTSUP;
}

int amdgpu_va_range_query(amdgpu_device_handle dev,
			  enum amdgpu_gpu_va_range type, uint64_t *start,
			  uint64_t *end)
{
	(void)dev;
	(void)type;
	(void)start;
	(void)end;
	return -ENOTSUP;
}

/* NULL, as libdrm answers for a device its table does not name. */
const char *amdgpu_get_marketing_name(amdgpu_device_handle dev)
{
	(void)dev;
	return NULL;
}
