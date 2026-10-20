#[derive(Debug, PartialEq)]
pub enum Action {
    HmiStart,
    Locker([u8; 2]),
    Reply(&'static [u8]),
    Nextion {
        bytes: &'static [u8],
        reply: &'static [u8],
    },
}

// No allocation: discard an oversized line completely before accepting another.
pub struct CommandBuffer {
    bytes: [u8; 16],
    len: usize,
    overflow: bool,
}

impl CommandBuffer {
    pub const fn new() -> Self {
        Self {
            bytes: [0; 16],
            len: 0,
            overflow: false,
        }
    }

    pub fn push(&mut self, byte: u8) -> Option<Action> {
        if byte == b'\r' || byte == b'\n' {
            let reply = if self.overflow {
                Some(Action::Reply(b"ERR line too long\r\n"))
            } else if self.len == 0 {
                None
            } else if &self.bytes[..self.len] == b"HMI START" {
                Some(Action::HmiStart)
            } else if &self.bytes[..self.len] == b"PING" {
                Some(Action::Reply(b"PONG\r\n"))
            } else if let Some(request) = match &self.bytes[..self.len] {
                b"OPEN 1" => Some([1, 1]),
                b"OPEN 2" => Some([1, 2]),
                b"CLOSE 1" => Some([2, 1]),
                b"CLOSE 2" => Some([2, 2]),
                b"STATUS" => Some([3, 0]),
                _ => None,
            } {
                Some(Action::Locker(request))
            } else if &self.bytes[..self.len] == b"DIM 30" {
                Some(Action::Nextion {
                    bytes: b"dim=30\xff\xff\xff",
                    reply: b"DIM command sent\r\n",
                })
            } else if &self.bytes[..self.len] == b"DIM 80" {
                Some(Action::Nextion {
                    bytes: b"dim=80\xff\xff\xff",
                    reply: b"DIM command sent\r\n",
                })
            } else if &self.bytes[..self.len] == b"TOUCH ON" {
                Some(Action::Nextion {
                    bytes: b"sendxy=1\xff\xff\xff",
                    reply: b"Touch reporting enabled\r\n",
                })
            } else {
                Some(Action::Reply(b"ERR unknown command\r\n"))
            };
            self.len = 0;
            self.overflow = false;
            return reply;
        }

        if !self.overflow {
            if self.len < self.bytes.len() {
                self.bytes[self.len] = byte;
                self.len += 1;
            } else {
                self.overflow = true;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, CommandBuffer};
    #[test]
    fn hmi_requires_explicit_usb_start_command() {
        let mut buffer = CommandBuffer::new();
        for byte in b"HMI START" {
            assert_eq!(buffer.push(*byte), None);
        }
        assert_eq!(buffer.push(b'\n'), Some(Action::HmiStart));
    }
    #[test]
    fn locker_commands_map_to_protocol() {
        for (line, request) in [
            ("OPEN 1", [1, 1]),
            ("OPEN 2", [1, 2]),
            ("CLOSE 1", [2, 1]),
            ("CLOSE 2", [2, 2]),
            ("STATUS", [3, 0]),
        ] {
            let mut buffer = CommandBuffer::new();
            for byte in line.bytes() {
                buffer.push(byte);
            }
            assert_eq!(buffer.push(b'\n'), Some(Action::Locker(request)));
        }
    }

    #[test]
    fn fragmented_ping_and_crlf() {
        let mut buffer = CommandBuffer::new();
        for byte in b"PING" {
            assert_eq!(buffer.push(*byte), None);
        }
        assert_eq!(buffer.push(b'\r'), Some(Action::Reply(&b"PONG\r\n"[..])));
        assert_eq!(buffer.push(b'\n'), None);
        assert_eq!(buffer.push(b'\n'), None);
    }

    #[test]
    fn unknown_and_full_length_lines() {
        let mut buffer = CommandBuffer::new();
        for line in [&b"ping"[..], &b"1234567890123456"[..]] {
            for byte in line {
                assert_eq!(buffer.push(*byte), None);
            }
            assert_eq!(
                buffer.push(b'\n'),
                Some(Action::Reply(&b"ERR unknown command\r\n"[..]))
            );
        }
    }

    #[test]
    fn overflow_discards_ping_suffix_and_recovers() {
        let mut buffer = CommandBuffer::new();
        for byte in b"12345678901234567PING" {
            assert_eq!(buffer.push(*byte), None);
        }
        assert_eq!(
            buffer.push(b'\n'),
            Some(Action::Reply(&b"ERR line too long\r\n"[..]))
        );
        for byte in b"PING" {
            buffer.push(*byte);
        }
        assert_eq!(buffer.push(b'\n'), Some(Action::Reply(&b"PONG\r\n"[..])));
    }

    #[test]
    fn nextion_commands_have_exact_terminators() {
        for (input, bytes, reply) in [
            (
                &b"DIM 30"[..],
                &b"dim=30\xff\xff\xff"[..],
                &b"DIM command sent\r\n"[..],
            ),
            (
                &b"DIM 80"[..],
                &b"dim=80\xff\xff\xff"[..],
                &b"DIM command sent\r\n"[..],
            ),
            (
                &b"TOUCH ON"[..],
                &b"sendxy=1\xff\xff\xff"[..],
                &b"Touch reporting enabled\r\n"[..],
            ),
        ] {
            let mut buffer = CommandBuffer::new();
            for &byte in input {
                assert_eq!(buffer.push(byte), None);
            }
            assert_eq!(buffer.push(b'\n'), Some(Action::Nextion { bytes, reply }));
        }
    }
}
