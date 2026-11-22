#[cfg(target_os = "android")]
use android_activity::AndroidApp;

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(_app: AndroidApp) {
    main();
}

#[cfg(target_os = "android")]
include!("main.rs");
