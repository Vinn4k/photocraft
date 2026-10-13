//! PhotoCraft on Android: the egui shell in a NativeActivity (eframe + winit's `android-activity`),
//! drawing with wgpu on Vulkan. The phone and tablet layouts come from `ui-egui`'s `mobile`
//! module, chosen by the screen size.
//!
//! Files: Open, Save As and Export go through PhotoCraft's own file browser (`files.rs`) over the
//! phone's shared storage, with Android's "All files access" (`system.rs` asks for it). The
//! system bars and the keyboard are kept clear as egui's safe area. Preferences live in the app's
//! private storage.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

// `android::android_main` is exported by name (`no_mangle`) for android-activity to call.
#[cfg(target_os = "android")]
mod android;
// Platform-neutral, so its tests run on the build machine.
#[cfg(any(target_os = "android", test))]
mod files;
#[cfg(target_os = "android")]
mod system;
