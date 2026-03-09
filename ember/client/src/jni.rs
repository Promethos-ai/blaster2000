//! JNI bindings for Android. Built only with the "android" feature.

use jni::objects::{JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;

/// Ask the AI a question via the ember server.
/// Called from Kotlin: EmberClient.ask(serverAddr: String, prompt: String): String
#[no_mangle]
pub extern "system" fn Java_com_ember_android_EmberClient_ask(
    mut env: JNIEnv,
    _class: JClass,
    server_addr: JString,
    prompt: JString,
) -> jstring {
    let addr_str: String = match env.get_string(&server_addr) {
        Ok(s) => s.into(),
        Err(e) => return env.new_string(format!("Error: {}", e)).unwrap().into_raw(),
    };
    let prompt_str: String = match env.get_string(&prompt) {
        Ok(s) => s.into(),
        Err(e) => return env.new_string(format!("Error: {}", e)).unwrap().into_raw(),
    };

    match crate::ask_ai(&addr_str, &prompt_str) {
        Ok(response) => env.new_string(response).unwrap().into_raw(),
        Err(e) => env.new_string(format!("Error: {}", e)).unwrap().into_raw(),
    }
}
