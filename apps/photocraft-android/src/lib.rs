//! PhotoCraft on Android: the egui shell in a NativeActivity (eframe + winit's `android-activity`),
//! drawing with wgpu on Vulkan. The phone and tablet layouts come from `ui-egui`'s `mobile`
//! module, chosen by the screen size.
//!
//! Files (until the system file picker, phase 5 of docs/android-mobile.md): Save, Save As and
//! Export write to the app's folder on shared storage (`Android/data/ai.storyteller.photocraft/
//! files`), and File › Open reports that opening files is not available yet. Preferences live in
//! the app's private storage.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

// `android::android_main` is exported by name (`no_mangle`) for android-activity to call.
#[cfg(target_os = "android")]
mod android;
