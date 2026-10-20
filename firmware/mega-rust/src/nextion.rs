#[derive(Debug, PartialEq)]
pub struct Touch {
    pub x: u16,
    pub y: u16,
    pub pressed: bool,
}

// A sliding nine-byte window also recovers from noise or a damaged frame.
// Coordinates are binary and may contain 0xff; only positions 6..9 terminate.
#[cfg(test)] // Retain the Phase 3A parser as a regression-test fixture.
pub struct TouchParser {
    bytes: [u8; 9],
    len: usize,
    idle_ticks: u8,
}

#[cfg(test)]
impl TouchParser {
    pub const fn new() -> Self {
        Self {
            bytes: [0; 9],
            len: 0,
            idle_ticks: 0,
        }
    }
    pub fn idle_tick(&mut self) {
        self.idle_ticks = self.idle_ticks.saturating_add(1);
        if self.idle_ticks >= 100 {
            self.len = 0;
        }
    }
    pub fn push(&mut self, byte: u8) -> Option<Touch> {
        self.idle_ticks = 0;
        if self.len == 9 {
            self.bytes.copy_within(1..9, 0);
            self.len = 8;
        }
        self.bytes[self.len] = byte;
        self.len += 1;
        if self.len != 9
            || !matches!(self.bytes[0], 0x67 | 0x68)
            || self.bytes[5] > 1
            || self.bytes[6..9] != [0xff; 3]
        {
            return None;
        }
        let event = Touch {
            x: u16::from_be_bytes([self.bytes[1], self.bytes[2]]),
            y: u16::from_be_bytes([self.bytes[3], self.bytes[4]]),
            pressed: self.bytes[5] == 1,
        };
        self.len = 0;
        Some(event)
    }
}

pub struct TouchLine {
    pub bytes: [u8; 48],
    pub len: usize,
}
impl core::fmt::Write for TouchLine {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        if text.len() > self.bytes.len() - self.len {
            return Err(core::fmt::Error);
        }
        self.bytes[self.len..self.len + text.len()].copy_from_slice(text.as_bytes());
        self.len += text.len();
        Ok(())
    }
}
impl Touch {
    pub fn line(&self) -> TouchLine {
        use core::fmt::Write;
        let mut line = TouchLine {
            bytes: [0; 48],
            len: 0,
        };
        // Even maximal u16 coordinates fit the fixed buffer.
        let _ = write!(
            &mut line,
            "TOUCH x={} y={} state={}\r\n",
            self.x,
            self.y,
            if self.pressed { "PRESS" } else { "RELEASE" }
        );
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn awake_sleep_press_release_and_back_to_back() {
        let mut parser = TouchParser::new();
        for header in [0x67, 0x68] {
            for state in [0, 1] {
                let packet = [header, 1, 84, 0, 210, state, 255, 255, 255];
                for &byte in &packet[..8] {
                    assert_eq!(parser.push(byte), None);
                }
                assert_eq!(
                    parser.push(255),
                    Some(Touch {
                        x: 340,
                        y: 210,
                        pressed: state == 1
                    })
                );
            }
        }
    }
    #[test]
    fn noise_bad_state_bad_terminator_and_recovery() {
        let mut p = TouchParser::new();
        for &byte in &[
            0x01, 255, 255, 255, 0x67, 0, 1, 0, 2, 2, 255, 255, 255, 0x68, 0, 1, 0, 2, 1, 255, 254,
            255,
        ] {
            assert_eq!(p.push(byte), None);
        }
        let packet = [0x67, 0, 255, 1, 0, 1, 255, 255, 255];
        for &byte in &packet[..8] {
            assert_eq!(p.push(byte), None);
        }
        assert_eq!(
            p.push(255),
            Some(Touch {
                x: 255,
                y: 256,
                pressed: true
            })
        );
    }
    #[test]
    fn partial_packet_expires_and_format_matches() {
        let mut p = TouchParser::new();
        for byte in [0x67, 0, 1] {
            p.push(byte);
        }
        for _ in 0..100 {
            p.idle_tick();
        }
        for byte in [0, 2, 1, 255, 255, 255] {
            assert_eq!(p.push(byte), None);
        }
        let line = Touch {
            x: 340,
            y: 210,
            pressed: true,
        }
        .line();
        assert_eq!(
            &line.bytes[..line.len],
            b"TOUCH x=340 y=210 state=PRESS\r\n"
        );
        let line = Touch {
            x: 65535,
            y: 65535,
            pressed: false,
        }
        .line();
        assert_eq!(
            &line.bytes[..line.len],
            b"TOUCH x=65535 y=65535 state=RELEASE\r\n"
        );
    }
}
