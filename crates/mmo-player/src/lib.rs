#[cfg(target_os = "android")]
use android_activity::AndroidApp;

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    run(app.internal_data_path());
}

#[cfg(target_os = "android")]
include!("main.rs");
