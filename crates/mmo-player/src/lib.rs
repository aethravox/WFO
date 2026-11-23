#[cfg(target_os = "android")]
use android_activity::AndroidApp;
#[cfg(target_os = "android")]
use winit::platform::android::EventLoopBuilderExtAndroid;

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: AndroidApp) {
    let data_dir = app.internal_data_path();
    let event_loop = EventLoop::builder()
        .with_android_app(app)
        .build()
        .expect("create android winit event loop");
    run(data_dir, event_loop);
}

#[cfg(target_os = "android")]
include!("main.rs");
