<h1 align="center">
  <img src="assets/stela-icon.png" alt="" width="96"><br>
  Stela
</h1>

A general purpose Apple Silicon layer for x86-64 PCs. It is a UEFI boot
program with an arm64-to-x86-64 translator, a Metal driver for NVIDIA GPUs and
a model of Apple silicon hardware. macOS and most apps start, but it is slow and needs
much more testing, as of v0.1.0

> [!WARNING]
Stela is very much still in development. Your current chances of it booting are low, major hardware support/fixes are on the way.

> [!NOTE]
> This fork, [LBNM-ONE/Stela](https://github.com/LBNM-ONE/Stela), is porting
> Stela's GPU support to the AMD Radeon RX 9070 (Navi 48, RDNA4). Its default
> branch carries the work in progress, and [HANDOFF.md](HANDOFF.md), in
> Italian, records each step and what is and is not verified. Stela itself is
> [tylertms/Stela](https://github.com/tylertms/Stela).

## Translator

An ahead-of-time and just-in-time compiler translates arm64e code to x86-64.
It covers more than 3,000 AArch64 instruction encodings, maps guest registers
to host registers and caches translated code. It runs on any x86-64-v2 CPU,
Intel or AMD.

## NVIDIA driver

The Metal driver supports GTX 16 and RTX 20 series GPUs and newer. It compiles
AIR, Metal's shader IR, directly to NIR, Mesa's IR, and supports the Metal3
and Apple7 GPU families.

## AMD driver (in progress)

This fork compiles Linux's amdgpu unchanged into the Linux environment of
`stela.efi`, as Stela does nouveau. An image built with
`cargo xtask boot build --release --amdgpu` claims an RX 9070 or 9070 XT
(PCI 1002:7550) and loads the firmware amdgpu asks for, pinned from
linux-firmware, from the USB stick's firmware bundle. amdgpu then binds the
GPU with neither display nor Metal device, so with that image macOS starts
only in single-user mode, and the monitor must be on the motherboard's
outputs. None of this has run on the hardware yet. RADV, Mesa's Vulkan driver
for AMD, the Metal device on top of it and display support are still to come.

## Requirements

- An x86-64-v2 CPU (about 2011 or newer), UEFI with Secure Boot off, 8 GB of
  RAM.
- An NVIDIA GTX 16 or RTX 20 series GPU or newer. The RX 9070 support of this
  fork cannot run macOS yet.
- A USB stick of 8 GB or more.
- An empty GPT partition of at least 60 GB.
- Network access during the install, which downloads macOS.

Stela currently models the Mac mini (M2) and runs macOS 27.

## Install

1. Download `stela` from the
   [releases page](https://github.com/tylertms/Stela/releases).
2. Run `sudo ./stela setup` (Linux, macOS) or double-click `stela.exe`
   (Windows).
3. Choose the USB stick, the macOS version and the Mac model, then confirm.
   Setup erases the USB stick.
4. Boot the PC from the USB stick.

To update a USB stick made with an older version, download the new `stela`,
plug the stick in and run `sudo ./stela setup update stela` (Linux, macOS) or
`stela.exe setup update stela` (Windows).

## Build

You need [Rust](https://rustup.rs) and LLVM with Clang 21 or newer.

```sh
git clone https://github.com/LBNM-ONE/Stela
cd Stela
cargo xtask dist
```

This writes `target/release/stela`. The first build fetches and compiles
Linux, Mesa and LLVM sources and takes a long time.

The releases are Stela's and carry no AMD support. The RX 9070 test image is
built with amdgpu instead of NVK:

```sh
cargo xtask boot build --release --amdgpu
```

HANDOFF.md says how to put it on a USB stick and read its boot log.

## Legal

Stela contains no Apple software. You are responsible for following Apple's
software license agreement and terms of use.

Stela's code is GPL-2.0-or-later. See [LICENSE](LICENSE) and [NOTICE](NOTICE).

## Community

[Discord Server](https://discord.gg/aSkwrJEMYc)
