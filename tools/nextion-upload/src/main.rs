#[cfg(feature = "hardware-upload")]
mod hardware;
mod hash;
mod protocol;
mod sim;

use std::io::Read;

const WRITE_CONFIRMATION: &str = "REPLACE_NEXTION_CONTENT_NO_FACTORY_BACKUP";

fn validate(bytes: &[u8], size: usize, expected_hash: &str) -> Result<String, String> {
    if size == 0 || size > 131_072_000 || bytes.len() != size {
        return Err(format!(
            "TFT size mismatch/invalid: expected {size}, actual {}",
            bytes.len()
        ));
    }
    if expected_hash.len() != 64 || !expected_hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("SHA-256 must contain exactly 64 hexadecimal characters".into());
    }
    let actual = hash::sha256(bytes)?;
    if !actual.eq_ignore_ascii_case(expected_hash) {
        return Err(format!("SHA-256 mismatch: actual {actual}"));
    }
    Ok(actual)
}

fn run(args: &[String]) -> Result<(), String> {
    if args.is_empty() || args == ["--help"] {
        println!("nextion-upload simulate --file PATH --size BYTES --sha256 HEX [--scenario success|missing-ready|missing-ack|bad-ack|disconnect|late-ack|missing-restart|wrong-model|wrong-serial|wrong-address]\nReal upload requires hardware-upload build feature and explicit --confirm-write token. COM3 / 9600 / 8N1 are fixed. Simulation never opens a port.");
        return Ok(());
    }
    let mode = &args[0];
    if mode != "simulate" && mode != "upload" {
        return Err("Mode must be simulate or upload".into());
    }
    let mut file = None;
    let mut size = None;
    let mut sha = None;
    let mut scenario = "success";
    let mut confirm = None;
    let mut seen = std::collections::HashSet::new();
    for pair in args[1..].chunks(2) {
        if pair.len() != 2 || !seen.insert(pair[0].as_str()) {
            return Err("Missing value or duplicate option".into());
        }
        match pair[0].as_str() {
            "--file" => file = Some(pair[1].as_str()),
            "--size" => size = Some(pair[1].parse::<usize>().map_err(|_| "Invalid --size")?),
            "--sha256" => sha = Some(pair[1].as_str()),
            "--scenario" if mode == "simulate" => scenario = &pair[1],
            "--confirm-write" if mode == "upload" => confirm = Some(pair[1].as_str()),
            _ => return Err(format!("Unknown/inapplicable option: {}", pair[0])),
        }
    }
    if mode == "upload" {
        if confirm != Some(WRITE_CONFIRMATION) {
            return Err("Physical writing not enabled: explicit content-replacement confirmation is required".into());
        }
        if !cfg!(feature = "hardware-upload") {
            return Err("This simulation-only build contains no serial backend".into());
        }
    }
    let file = file.ok_or("--file required")?;
    let size = size.ok_or("--size required")?;
    let sha = sha.ok_or("--sha256 required")?;
    if size == 0 || size > 131_072_000 {
        return Err("File size must be 1..131072000".into());
    }
    // Load once; validated bytes are exactly those sent. A later file change cannot replace them.
    let mut bytes = Vec::new();
    std::fs::File::open(file)
        .map_err(|e| e.to_string())?
        .take(size as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let actual = validate(&bytes, size, sha)?;
    println!("Validated {size} bytes; SHA-256 {actual}; fixed COM3 / 9600 / 8N1");
    if mode == "simulate" {
        let mut device = sim::Sim::new(&bytes, sim::Scenario::parse(scenario)?);
        let result = protocol::upload(&mut device, &bytes);
        println!(
            "SIMULATION ONLY: payload received={}, writes={}, max chunk={}; no serial port opened",
            device.received.len(),
            device.writes,
            device.max_chunk
        );
        let report = result?;
        println!(
            "SIMULATION PASS: {} bytes, {} blocks, {:.3} virtual seconds; ready frame received",
            report.bytes,
            report.blocks,
            report.elapsed_ms as f64 / 1000.0
        );
    } else {
        #[cfg(feature = "hardware-upload")]
        {
            eprintln!("WARNING: this operation replaces screen content; no factory backup exists. No automatic recovery/retry.");
            let mut device = hardware::Hardware::open()?;
            let report = protocol::upload(&mut device, &bytes)?;
            println!("All {} bytes in {} blocks acknowledged; screen ready, elapsed {:.1}s. On-device UI still requires observation.", report.bytes, report.blocks, report.elapsed_ms as f64 / 1000.0);
        }
    }
    Ok(())
}

fn main() {
    if let Err(e) = run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        eprintln!("STOP: {e}. No automatic retry or resume.");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests;
