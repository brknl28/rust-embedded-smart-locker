pub const BAUD: u64 = 9600;
pub const BLOCK: usize = 4096;
pub const CHUNK: usize = 16;
pub const GAP_MS: u64 = 10;
pub const MODEL: &str = "NX8048P070_011R";
pub const SERIAL: &str = "953E3401412AB238";
pub const ACK_TIMEOUT_MS: u64 = 3000;
pub const RESTART_TIMEOUT_MS: u64 = 60_000;

/// Absolute virtual/real monotonic milliseconds make the same engine testable offline.
pub trait Transport {
    fn now(&self) -> u64;
    fn sleep(&mut self, ms: u64);
    fn write(&mut self, bytes: &[u8]) -> Result<(), String>;
    fn drain(&mut self, deadline: u64) -> Result<(), String>;
    fn read(&mut self, deadline: u64) -> Result<u8, String>;
    fn progress(&mut self, _bytes: usize, _total: usize) {}
}

pub fn paced_send(t: &mut impl Transport, bytes: &[u8]) -> Result<(), String> {
    for chunk in bytes.chunks(CHUNK) {
        let start = t.now();
        t.write(chunk)?;
        t.drain(start + 2000)?;
        // 8N1: ten bits per byte. Wait for nominal wire time plus a safety gap.
        // OS queue-empty is NOT a guarantee that the AVR/Nextion received the bytes.
        let due = start + (chunk.len() as u64 * 10 * 1000).div_ceil(BAUD) + GAP_MS;
        if due > t.now() {
            t.sleep(due - t.now());
        }
    }
    Ok(())
}

fn frame(t: &mut impl Transport, deadline: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    loop {
        bytes.push(t.read(deadline)?);
        if bytes.ends_with(&[0xff; 3]) {
            bytes.truncate(bytes.len() - 3);
            return Ok(bytes);
        }
        if bytes.len() >= 512 {
            return Err("Oversized or unterminated device response".into());
        }
    }
}

pub fn verify_device(t: &mut impl Transport) -> Result<(), String> {
    paced_send(t, b"connect\xff\xff\xff")?;
    let deadline = t.now() + 5000;
    // Factory demo may emit framed touch/startup data. Bound all such traffic.
    for _ in 0..8 {
        let bytes = frame(t, deadline)?;
        if !bytes.starts_with(b"comok ") {
            continue;
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| "Non-ASCII comok response")?;
        let fields: Vec<_> = text[6..].split(',').collect();
        if fields.len() != 7 || fields[2] != MODEL || fields[5] != SERIAL {
            return Err(format!("Unexpected device identity: {text}"));
        }
        if fields[1].rsplit_once('-').map(|(_, addr)| addr) != Some("0") {
            return Err("Nonzero/unknown device address is unsupported".into());
        }
        return Ok(());
    }
    Err("No valid comok response within bounded frame limit".into())
}

fn ack(t: &mut impl Transport, context: &str) -> Result<(), String> {
    let byte = t
        .read(t.now() + ACK_TIMEOUT_MS)
        .map_err(|e| format!("{context}: {e}"))?;
    if byte != 0x05 {
        return Err(format!("{context}: expected ACK 05, got {byte:02X}"));
    }
    Ok(())
}

#[derive(Debug)]
pub struct Report {
    pub bytes: usize,
    pub blocks: usize,
    pub elapsed_ms: u64,
}

/// No retries, resume, speed changes, or debug writes to the transport.
pub fn upload(t: &mut impl Transport, payload: &[u8]) -> Result<Report, String> {
    if payload.is_empty() {
        return Err("Empty TFT is not allowed".into());
    }
    let start = t.now();
    verify_device(t)?;
    let mut command = format!("whmi-wri {},9600,0", payload.len()).into_bytes();
    command.extend_from_slice(&[0xff; 3]);
    paced_send(t, &command)?;
    ack(t, "Upload ready")?;
    let mut blocks = 0;
    for block in payload.chunks(BLOCK) {
        paced_send(t, block)?;
        ack(t, &format!("Block {}", blocks + 1))?;
        blocks += 1;
        t.progress((blocks * BLOCK).min(payload.len()), payload.len());
    }
    let deadline = t.now() + RESTART_TIMEOUT_MS;
    // Known power-on frame 00 00 00 FF FF FF may precede readiness.
    for _ in 0..8 {
        let response = frame(t, deadline).map_err(|e| {
            format!("All blocks acknowledged; screen readiness unconfirmed: {e}. Do not retry automatically")
        })?;
        if response == [0x88] {
            return Ok(Report {
                bytes: payload.len(),
                blocks,
                elapsed_ms: t.now() - start,
            });
        }
        if response != [0, 0, 0] {
            return Err(format!(
                "All blocks acknowledged; unexpected restart frame {response:02X?}"
            ));
        }
    }
    Err("All blocks acknowledged; no screen readiness within frame limit".into())
}
