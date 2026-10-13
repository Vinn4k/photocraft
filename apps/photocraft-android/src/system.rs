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

/// May PhotoCraft read and write the phone's files? Android 11+: "All files access" (or, from
/// Android 13, at least the photos permission); before: the storage permission.
pub fn has_file_access() -> bool {
    call(|env, activity| {
        let sdk = sdk_int(env)?;
        if sdk >= 30 && env.call_static_method("android/os/Environment", "isExternalStorageManager", "()Z", &[])?.z()? {
            return Ok(true);
        }
        let permission = if sdk >= 33 {
            "android.permission.READ_MEDIA_IMAGES"
        } else if sdk >= 30 {
            return Ok(false);
        } else {
            "android.permission.WRITE_EXTERNAL_STORAGE"
        };
        let permission = env.new_string(permission)?;
        Ok(env.call_method(activity, "checkSelfPermission", "(Ljava/lang/String;)I", &[JValue::from(&permission)])?.i()? == 0)
    })
    .unwrap_or(false)
}

/// Open a Settings page (`action`, for this app when `for_app`).
fn open_settings(env: &mut JNIEnv, activity: &JObject, action: &str, for_app: bool) -> jni::errors::Result<()> {
    let action = env.new_string(action)?;
    let intent = env.new_object("android/content/Intent", "(Ljava/lang/String;)V", &[JValue::from(&action)])?;
    if for_app {
        let text = env.new_string(format!("package:{PACKAGE}"))?;
        let uri = env.call_static_method("android/net/Uri", "parse", "(Ljava/lang/String;)Landroid/net/Uri;", &[JValue::from(&text)])?.l()?;
        env.call_method(&intent, "setData", "(Landroid/net/Uri;)Landroid/content/Intent;", &[JValue::from(&uri)])?;
    }
    // FLAG_ACTIVITY_NEW_TASK: Settings in its own task, whatever thread asks.
    env.call_method(&intent, "addFlags", "(I)Landroid/content/Intent;", &[JValue::Int(0x1000_0000)])?;
    env.call_method(activity, "startActivity", "(Landroid/content/Intent;)V", &[JValue::from(&intent)])?;
    Ok(())
}

fn request_permissions(env: &mut JNIEnv, activity: &JObject, names: &[&str]) -> jni::errors::Result<()> {
    let string = env.find_class("java/lang/String")?;
    let permissions = env.new_object_array(names.len() as i32, &string, JObject::null())?;
    for (i, name) in names.iter().enumerate() {
        let name = env.new_string(name)?;
        env.set_object_array_element(&permissions, i as i32, name)?;
    }
    env.call_method(activity, "requestPermissions", "([Ljava/lang/String;I)V", &[JValue::from(&permissions), JValue::Int(1)])?;
    Ok(())
}

/// Ask for file access. Android 11+ opens PhotoCraft's "All files access" switch in Settings
/// (else the list of apps with it, else PhotoCraft's app info); Android 13+ also asks for the
/// photos permission when no Settings page opens. Before 11, the system asks for the storage
/// permission. Returns which way it asked.
pub fn request_file_access() -> Result<Asked, String> {
    call(|env, activity| {
        let sdk = sdk_int(env)?;
        if sdk < 30 {
            request_permissions(env, activity, &["android.permission.READ_EXTERNAL_STORAGE", "android.permission.WRITE_EXTERNAL_STORAGE"])?;
            return Ok(Asked::Permission);
        }
        let pages = [
            ("android.settings.MANAGE_APP_ALL_FILES_ACCESS_PERMISSION", true, Asked::AllFilesSwitch),
            ("android.settings.MANAGE_ALL_FILES_ACCESS_PERMISSION", false, Asked::AllFilesList),
        ];
        for (action, for_app, asked) in pages {
            match open_settings(env, activity, action, for_app) {
                Ok(()) => return Ok(asked),
                Err(e) => {
                    log::warn!("photocraft: {action}: {e}");
                    if env.exception_check()? {
                        env.exception_clear()?;
                    }
                }
            }
        }
        if sdk >= 33 {
            request_permissions(env, activity, &["android.permission.READ_MEDIA_IMAGES"])?;
            return Ok(Asked::Permission);
        }
        open_settings(env, activity, "android.settings.APPLICATION_DETAILS_SETTINGS", true)?;
        Ok(Asked::AppInfo)
    })
}

/// How [`request_file_access`] asked, to say what to do next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked {
    AllFilesSwitch,
    AllFilesList,
    AppInfo,
    Permission,
}
