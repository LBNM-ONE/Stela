// SPDX-License-Identifier: GPL-2.0-or-later
/*
 * compiler-rt's table behind __builtin_cpu_supports, which addrlib's swizzler
 * reads before it takes an AVX path. The image has no compiler-rt, whose
 * constructor fills the table from CPUID: this one stays zero, so every
 * feature reads as absent and addrlib swizzles with its portable code.
 */
struct __processor_model {
	unsigned int __cpu_vendor;
	unsigned int __cpu_type;
	unsigned int __cpu_subtype;
	unsigned int __cpu_features[1];
} __cpu_model;
