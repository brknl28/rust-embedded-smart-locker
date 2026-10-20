use crate::protocol::Transport;
use serialport::{ClearBuffer, SerialPort, SerialPortType};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

pub struct Hardware {
    port: Box<dyn SerialPort>,
    start: Instant,
}

impl Hardware {
    pub fn open() -> Result<Self, String> {
        let ports = serialport::available_ports().map_err(|e| e.to_string())?;
        let port = ports
            .iter()
            .find(|p| p.port_name.eq_ignore_ascii_case("COM3"))
            .ok_or("COM3 not present; no fallback port will be tried")?;
        match &port.port_type {
            SerialPortType::UsbPort(info) if info.vid == 0x1a86 && info.pid == 0x7523 => (),
            _ => return Err("COM3 does not report expected CH340 VID 1A86 / PID 7523".into()),
        }
        let mut port = serialport::new("COM3", 9600)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .timeout(Duration::from_millis(250))
            .open()
            .map_err(|e| e.to_string())?;
        port.write_data_terminal_ready(true)
            .map_err(|e| e.to_string())?;
        std::thread::sleep(Duration::from_secs(3));
        port.clear(ClearBuffer::All).map_err(|e| e.to_string())?;
        Ok(Self {
            port,
            start: Instant::now(),
        })
    }
}

impl Transport for Hardware {
    fn progress(&mut self, bytes: usize, total: usize) {
        if bytes % (16 * crate::protocol::BLOCK) == 0 || bytes == total {
            println!(
                "Acknowledged {bytes}/{total} bytes ({:.1}%)",
                bytes as f64 * 100.0 / total as f64
            );
        }
    }
    fn now(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
    fn sleep(&mut self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }
    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        // Read polling may leave a much shorter timeout near its deadline.
        self.port
            .set_timeout(Duration::from_millis(250))
            .map_err(|e| e.to_string())?;
        self.port
            .write_all(bytes)
            .map_err(|e| format!("Serial write failed: {e}"))
    }
    fn drain(&mut self, deadline: u64) -> Result<(), String> {
        loop {
            if self.port.bytes_to_write().map_err(|e| e.to_string())? == 0 {
                return Ok(());
            }
            if self.now() >= deadline {
                return Err("Serial output queue timeout".into());
            }
            self.sleep(2);
        }
    }
    fn read(&mut self, deadline: u64) -> Result<u8, String> {
        while self.now() < deadline {
            self.port
                .set_timeout(Duration::from_millis(
                    deadline.saturating_sub(self.now()).min(100).max(1),
                ))
                .map_err(|e| e.to_string())?;
            let mut byte = [0];
            match self.port.read(&mut byte) {
                Ok(1) => return Ok(byte[0]),
                Ok(_) => return Err("Serial port returned EOF".into()),
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) =>
                {
                    ()
                }
                Err(e) => return Err(format!("Serial receive failed: {e}")),
            }
        }
        Err("Serial receive timeout".into())
    }
}

impl Drop for Hardware {
    fn drop(&mut self) {
        // Best effort discard of OS-queued output. Bytes already on the wire cannot be recalled.
        let _ = self.port.clear(ClearBuffer::Output);
        // Closing the handle is automatic. Never reopen, retry, or toggle baud on failure.
    }
}
