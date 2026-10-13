//! What PhotoCraft asks Android through JNI (robius-android-env hands over the activity behind a
//! safe API): the system bars' insets, and access to the phone's files.

use jni::JNIEnv;
use jni::objects::{JObject, JValue};
use robius_android_env::with_activity;

const PACKAGE: &str = "ai.storyteller.photocraft";

/// Insets in physical pixels: left, top, right, bottom.
pub type Insets = [i32; 4];

/// Run `f` with the activity, clearing a Java exception it leaves (a failed call must not poison
/// the next one).
fn call<R>(f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<R>) -> Result<R, String> {
    let result = with_activity(|env, activity| {
        let r = f(env, activity);
        if r.is_err() && env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
        r
    });
    match result {
        Ok(Ok(r)) => Ok(r),
        Ok(Err(e)) | Err(e) => Err(e.to_string()),
    }
}

fn sdk_int(env: &mut JNIEnv) -> jni::errors::Result<i32> {
    env.get_static_field("android/os/Build$VERSION", "SDK_INT", "I")?.i()
}

/// The status bar, navigation bar, display cutout and keyboard, as the window's insets.
pub fn insets() -> Result<Insets, String> {
    call(|env, activity| {
        let window = env.call_method(activity, "getWindow", "()Landroid/view/Window;", &[])?.l()?;
        let decor = env.call_method(&window, "getDecorView", "()Landroid/view/View;", &[])?.l()?;
        let insets = env.call_method(&decor, "getRootWindowInsets", "()Landroid/view/WindowInsets;", &[])?.l()?;
        if insets.is_null() {
            return Ok([0; 4]);
        }
        if sdk_int(env)? >= 30 {
            let ty = "android/view/WindowInsets$Type";
            let mut mask = 0;
            for kind in ["systemBars", "displayCutout", "ime"] {
                mask |= env.call_static_method(ty, kind, "()I", &[])?.i()?;
            }
            let i = env.call_method(&insets, "getInsets", "(I)Landroid/graphics/Insets;", &[JValue::Int(mask)])?.l()?;
            let mut side = |name| env.get_field(&i, name, "I").and_then(|v| v.i());
            Ok([side("left")?, side("top")?, side("right")?, side("bottom")?])
        } else {
            let mut side = |name| env.call_method(&insets, name, "()I", &[]).and_then(|v| v.i());
            Ok([side("getSystemWindowInsetLeft")?, side("getSystemWindowInsetTop")?, side("getSystemWindowInsetRight")?, side("getSystemWindowInsetBottom")?])
        }
    })
}

/// May PhotoCraft read and write the phone's files? (Android 11+: "All files access"; before:
/// the storage permission.)
pub fn has_file_access() -> bool {
    call(|env, activity| {
        if sdk_int(env)? >= 30 {
            env.call_static_method("android/os/Environment", "isExternalStorageManager", "()Z", &[])?.z()
        } else {
            let permission = env.new_string("android.permission.WRITE_EXTERNAL_STORAGE")?;
            Ok(env.call_method(activity, "checkSelfPermission", "(Ljava/lang/String;)I", &[JValue::from(&permission)])?.i()? == 0)
        }
    })
    .unwrap_or(false)
}

/// Ask for file access: Android 11+ opens the "All files access" switch for PhotoCraft in
/// Settings; before, the system asks for the storage permission.
pub fn request_file_access() -> Result<(), String> {
    call(|env, activity| {
        if sdk_int(env)? >= 30 {
            let start = |env: &mut JNIEnv, action: &str, uri: Option<&str>| -> jni::errors::Result<()> {
                let action = env.new_string(action)?;
                let intent = env.new_object("android/content/Intent", "(Ljava/lang/String;)V", &[JValue::from(&action)])?;
                if let Some(uri) = uri {
                    let text = env.new_string(uri)?;
                    let uri = env.call_static_method("android/net/Uri", "parse", "(Ljava/lang/String;)Landroid/net/Uri;", &[JValue::from(&text)])?.l()?;
                    env.call_method(&intent, "setData", "(Landroid/net/Uri;)Landroid/content/Intent;", &[JValue::from(&uri)])?;
                }
                env.call_method(activity, "startActivity", "(Landroid/content/Intent;)V", &[JValue::from(&intent)])?;
                Ok(())
            };
            let app_page = format!("package:{PACKAGE}");
            if start(env, "android.settings.MANAGE_APP_ALL_FILES_ACCESS_PERMISSION", Some(&app_page)).is_err() {
                // Some phones have no per-app page: the list of apps instead.
                if env.exception_check()? {
                    env.exception_clear()?;
                }
                start(env, "android.settings.MANAGE_ALL_FILES_ACCESS_PERMISSION", None)?;
            }
        } else {
            let string = env.find_class("java/lang/String")?;
            let permissions = env.new_object_array(2, &string, JObject::null())?;
            for (i, name) in ["android.permission.READ_EXTERNAL_STORAGE", "android.permission.WRITE_EXTERNAL_STORAGE"].into_iter().enumerate() {
                let name = env.new_string(name)?;
                env.set_object_array_element(&permissions, i as i32, name)?;
            }
            env.call_method(activity, "requestPermissions", "([Ljava/lang/String;I)V", &[JValue::from(&permissions), JValue::Int(1)])?;
        }
        Ok(())
    })
}
