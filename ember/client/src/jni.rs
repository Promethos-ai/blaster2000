//! JNI bindings for Android. Built only with the "android" feature.

use std::panic;
use std::sync::mpsc;

use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::jstring;
use jni::JNIEnv;

enum StreamItem {
    Token(String),
    RichContent(String),
    Style(String),
    Layout(String),
    Audio(String),
    Complete(String),
}

fn parse_addr(env: &mut JNIEnv, addr: &JString) -> Result<String, String> {
    env.get_string(addr).map(|s| s.into()).map_err(|e| format!("{}", e))
}

fn parse_prompt(env: &mut JNIEnv, prompt: &JString) -> Result<String, String> {
    env.get_string(prompt).map(|s| s.into()).map_err(|e| format!("{}", e))
}

/// Ask the AI a question via the ember server.
/// Called from Kotlin: EmberClient.ask(serverAddr: String, prompt: String): String
#[no_mangle]
pub extern "system" fn Java_com_ember_android_EmberClient_ask(
    mut env: JNIEnv,
    _class: JClass,
    server_addr: JString,
    prompt: JString,
) -> jstring {
    let addr_str = match parse_addr(&mut env, &server_addr) {
        Ok(s) => s,
        Err(e) => return env.new_string(format!("Error: {}", e)).unwrap().into_raw(),
    };
    let prompt_str = match parse_prompt(&mut env, &prompt) {
        Ok(s) => s,
        Err(e) => return env.new_string(format!("Error: {}", e)).unwrap().into_raw(),
    };

    match crate::ask_ai(&addr_str, &prompt_str) {
        Ok(response) => env.new_string(response).unwrap().into_raw(),
        Err(e) => env.new_string(format!("Error: {}", e)).unwrap().into_raw(),
    }
}

/// Ask with streaming; calls callback.onToken() for each token.
/// Uses hostname for TLS SNI (required for pinggy/proxy connections).
/// Called from Kotlin: EmberClient.askStreaming(serverAddr, prompt, callback): String
#[no_mangle]
pub extern "system" fn Java_com_ember_android_EmberClient_askStreaming(
    mut env: JNIEnv,
    _class: JClass,
    server_addr: JString,
    prompt: JString,
    callback: JObject,
) -> jstring {
    let addr_str = match parse_addr(&mut env, &server_addr) {
        Ok(s) => s,
        Err(e) => return env.new_string(format!("Error: {}", e)).unwrap().into_raw(),
    };
    let prompt_str = match parse_prompt(&mut env, &prompt) {
        Ok(s) => s,
        Err(e) => return env.new_string(format!("Error: {}", e)).unwrap().into_raw(),
    };

    // Channel to receive tokens from streaming (avoids capturing JNIEnv in a Send closure)
    let (tx, rx) = mpsc::sync_channel::<Result<StreamItem, String>>(64);

    let addr_str = addr_str.clone();
    let prompt_str = prompt_str.clone();
    std::thread::spawn(move || {
        let res = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            crate::ask_ai_streaming_full(
                &addr_str,
                &prompt_str,
                |token| { let _ = tx.send(Ok(StreamItem::Token(token.to_string()))); },
                |content| { let _ = tx.send(Ok(StreamItem::RichContent(content.to_string()))); },
                |css| { let _ = tx.send(Ok(StreamItem::Style(css.to_string()))); },
                |json| { let _ = tx.send(Ok(StreamItem::Layout(json.to_string()))); },
                |text| { let _ = tx.send(Ok(StreamItem::Audio(text.to_string()))); },
            )
        }));
        match res {
            Ok(Ok(s)) => { let _ = tx.send(Ok(StreamItem::Complete(s))); }
            Ok(Err(e)) => { let _ = tx.send(Err(e)); }
            Err(_) => { let _ = tx.send(Err("The app encountered an unexpected error. Please try again.".to_string())); }
        }
    });

    let mut result = String::new();
    loop {
        match rx.recv() {
            Ok(Ok(StreamItem::Token(token))) => {
                if let Ok(jtoken) = env.new_string(&token) {
                    let jobj = JObject::from(jtoken);
                    let _ = env.call_method(
                        &callback,
                        "onToken",
                        "(Ljava/lang/String;)V",
                        &[JValue::Object(&jobj)],
                    );
                }
                result.push_str(&token);
            }
            Ok(Ok(StreamItem::RichContent(content))) => {
                if let Ok(j) = env.new_string(&content) {
                    let _ = env.call_method(&callback, "onRichContent", "(Ljava/lang/String;)V", &[JValue::Object(&JObject::from(j))]);
                }
            }
            Ok(Ok(StreamItem::Style(css))) => {
                if let Ok(j) = env.new_string(&css) {
                    let _ = env.call_method(&callback, "onStyle", "(Ljava/lang/String;)V", &[JValue::Object(&JObject::from(j))]);
                }
            }
            Ok(Ok(StreamItem::Layout(json))) => {
                if let Ok(j) = env.new_string(&json) {
                    let _ = env.call_method(&callback, "onLayout", "(Ljava/lang/String;)V", &[JValue::Object(&JObject::from(j))]);
                }
            }
            Ok(Ok(StreamItem::Audio(text))) => {
                if let Ok(j) = env.new_string(&text) {
                    let _ = env.call_method(&callback, "onAudio", "(Ljava/lang/String;)V", &[JValue::Object(&JObject::from(j))]);
                }
            }
            Ok(Ok(StreamItem::Complete(s))) => {
                result = s;
                break;
            }
            Ok(Err(e)) => return env.new_string(format!("Error: {}", e)).unwrap().into_raw(),
            Err(_) => break,
        }
    }

    env.new_string(result).unwrap().into_raw()
}
