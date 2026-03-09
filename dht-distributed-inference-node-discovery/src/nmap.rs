use anyhow::{Context, Result};
use std::process::Command;
use tracing::{debug, info, warn};

pub struct NmapScanner;

impl NmapScanner {
    pub fn new() -> Self {
        Self
    }

    /// Check if nmap is available
    pub fn is_available() -> bool {
        let output = if cfg!(target_os = "windows") {
            Command::new("nmap")
                .arg("--version")
                .output()
        } else {
            Command::new("nmap")
                .arg("--version")
                .output()
        };

        output.is_ok() && output.unwrap().status.success()
    }

    /// Scan UDP port to check if it's open/listening
    pub async fn scan_port(&self, host: &str, port: u16) -> Result<()> {
        if !Self::is_available() {
            warn!("nmap is not available. Install nmap to use port scanning.");
            return Ok(());
        }

        info!("Scanning UDP port {} on {}...", port, host);

        // Run nmap UDP scan
        let output = Command::new("nmap")
            .arg("-sU")  // UDP scan
            .arg("-p")   // Port specification
            .arg(port.to_string())
            .arg("--version-intensity")  // Service detection
            .arg("5")
            .arg(host)
            .output()
            .context("Failed to execute nmap")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        debug!("nmap stdout: {}", stdout);
        if !stderr.is_empty() {
            debug!("nmap stderr: {}", stderr);
        }

        // Parse output
        if stdout.contains("open") || stdout.contains("open|filtered") {
            println!("   ✓ Port {} is open or filtered", port);
            info!("Port {} appears to be open", port);
        } else if stdout.contains("closed") {
            println!("   ✗ Port {} is closed", port);
            warn!("Port {} is closed", port);
        } else if stdout.contains("filtered") {
            println!("   ⚠ Port {} is filtered (firewall may be blocking)", port);
            warn!("Port {} is filtered", port);
        } else {
            println!("   ? Port {} status unknown", port);
            info!("Could not determine port {} status", port);
        }

        // Check for service detection
        if stdout.contains("QUIC") || stdout.contains("quic") {
            println!("   ✓ QUIC service detected");
        }

        println!("\n   Full nmap output:");
        println!("   {}", stdout.lines().collect::<Vec<_>>().join("\n   "));

        Ok(())
    }

    /// Quick connectivity test
    pub async fn test_connectivity(&self, host: &str, port: u16) -> Result<bool> {
        if !Self::is_available() {
            return Ok(false);
        }

        let output = Command::new("nmap")
            .arg("-sU")
            .arg("-p")
            .arg(port.to_string())
            .arg("--max-retries")
            .arg("1")
            .arg("--host-timeout")
            .arg("5s")
            .arg(host)
            .output()
            .context("Failed to execute nmap")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.contains("open") || stdout.contains("open|filtered"))
    }
}

