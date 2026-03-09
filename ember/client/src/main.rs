//! Desktop binary for the ember QUIC client.

fn main() -> Result<(), String> {
    let server_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:4433".to_string());
    let prompt = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "What is 2 + 2?".to_string());

    println!("Asking: {}", prompt);
    let response = ember_native::ask_ai(&server_addr, &prompt)?;
    println!("Response: {}", response);
    Ok(())
}
