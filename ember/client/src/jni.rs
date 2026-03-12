//! JNI bindings for Android. Built only with the "android" feature.

use std::net::ToSocketAddrs;
use std::sync::mpsc;

use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::jstring;
use jni::JNIEnv;

enum StreamItem {
    Token(String),
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

    let addrs: Vec<std::net::SocketAddr> = match addr_str.to_socket_addrs() {
        Ok(a) => a.collect(),
        Err(e) => {
            let msg = e.to_string();
            let friendly = if msg.contains("lookup") || msg.contains("hostname") || msg.contains("no address") {
                "Could not resolve hostname. Check: (1) proxy reachable (e.g. eagleoneonline.ca), (2) device has internet, (3) hostname is correct."
            } else {
                &msg
            };
            return env.new_string(format!("Error: {}", friendly)).unwrap().into_raw();
        }
    };
    let server_addr = addrs
        .iter()
        .find(|a| a.is_ipv4())
        .copied()
        .or_else(|| addrs.into_iter().next())
        .ok_or_else(|| "Could not resolve address".to_string())
        .map_err(|e| format!("Error: {}", e));
    let server_addr = match server_addr {
        Ok(a) => a,
        Err(e) => return env.new_string(e).unwrap().into_raw(),
    };

    // Channel to receive tokens from streaming (avoids capturing JNIEnv in a Send closure)
    let (tx, rx) = mpsc::sync_channel::<Result<StreamItem, String>>(64);

    let server_addr = server_addr;
    let prompt_str = prompt_str.clone();
    std::thread::spawn(move || {
        let res = crate::ask_ai_addr_streaming(server_addr, &prompt_str, |token| {
            let _ = tx.send(Ok(StreamItem::Token(token.to_string())));
        });
        match res {
            Ok(s) => { let _ = tx.send(Ok(StreamItem::Complete(s))); }
            Err(e) => { let _ = tx.send(Err(e)); }
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
