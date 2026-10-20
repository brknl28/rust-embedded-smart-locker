use crate::nextion::Touch;

#[derive(Debug, PartialEq)]
pub enum Event {
    Ready,
    Component { page: u8, id: u8, pressed: bool },
    Page(u8),
    Touch(Touch),
}

// Frame lengths protect payload bytes from being interpreted as event headers.
pub struct Parser {
    bytes: [u8; 9],
    len: usize,
    target: usize,
    idle: u8,
    discard: bool,
    ff: u8,
}
impl Parser {
    pub const fn new() -> Self {
        Self {
            bytes: [0; 9],
            len: 0,
            target: 0,
            idle: 0,
            discard: false,
            ff: 0,
        }
    }
    pub fn idle_tick(&mut self) {
        self.idle = self.idle.saturating_add(1);
        if self.idle >= 100 {
            self.len = 0;
            self.discard = false;
            self.ff = 0;
        }
    }
    pub fn push(&mut self, byte: u8) -> Option<Event> {
        self.idle = 0;
        if self.discard {
            self.ff = if byte == 255 { self.ff + 1 } else { 0 };
            if self.ff == 3 {
                self.discard = false;
                self.ff = 0;
            }
            return None;
        }
        if self.len == 0 {
            self.target = match byte {
                0x65 => 7,
                0x66 => 5,
                0x67 | 0x68 => 9,
                0x71 => 8,
                0x88 => 4,
                _ => {
                    self.discard = true;
                    self.ff = if byte == 255 { 1 } else { 0 };
                    return None;
                }
            };
        }
        self.bytes[self.len] = byte;
        self.len += 1;
        if self.len != self.target {
            return None;
        }
        self.len = 0;
        if self.bytes[self.target - 3..self.target] != [255; 3] {
            return None;
        }
        match self.bytes[0] {
            0x88 => Some(Event::Ready),
            0x65 if self.bytes[3] <= 1 => Some(Event::Component {
                page: self.bytes[1],
                id: self.bytes[2],
                pressed: self.bytes[3] == 1,
            }),
            0x66 => Some(Event::Page(self.bytes[1])),
            0x67 | 0x68 if self.bytes[5] <= 1 => Some(Event::Touch(Touch {
                x: u16::from_be_bytes([self.bytes[1], self.bytes[2]]),
                y: u16::from_be_bytes([self.bytes[3], self.bytes[4]]),
                pressed: self.bytes[5] == 1,
            })),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_requires_complete_terminator_and_cannot_hide_in_numeric_payload() {
        let mut parser = Parser::new();
        for byte in [0x71, 0x88, 0, 0, 0, 255, 255, 255] {
            assert_eq!(parser.push(byte), None);
        }
        for byte in [0x88, 255, 255] {
            assert_eq!(parser.push(byte), None);
        }
        assert_eq!(parser.push(255), Some(Event::Ready));
    }
    #[test]
    fn ignored_binary_and_string_packets_cannot_inject_events() {
        let mut parser = Parser::new();
        for byte in [
            0x71, 0x65, 0, 3, 0, 255, 255, 255, 0x70, 0x65, 0, 3, 0, 255, 255, 255,
        ] {
            assert_eq!(parser.push(byte), None);
        }
        for byte in [0x65, 0, 3, 0, 255, 255] {
            assert_eq!(parser.push(byte), None);
        }
        assert_eq!(
            parser.push(255),
            Some(Event::Component {
                page: 0,
                id: 3,
                pressed: false
            })
        );
    }
    #[test]
    fn release_press_page_and_coordinates_are_distinct() {
        let mut parser = Parser::new();
        for (packet, expected) in [
            (
                &[0x65, 2, 5, 0, 255, 255, 255][..],
                Event::Component {
                    page: 2,
                    id: 5,
                    pressed: false,
                },
            ),
            (
                &[0x65, 0, 3, 1, 255, 255, 255][..],
                Event::Component {
                    page: 0,
                    id: 3,
                    pressed: true,
                },
            ),
            (&[0x66, 1, 255, 255, 255][..], Event::Page(1)),
            (
                &[0x67, 0x65, 0, 0x66, 255, 0, 255, 255, 255][..],
                Event::Touch(Touch {
                    x: 25856,
                    y: 26367,
                    pressed: false,
                }),
            ),
        ] {
            for &byte in &packet[..packet.len() - 1] {
                assert_eq!(parser.push(byte), None);
            }
            assert_eq!(parser.push(255), Some(expected));
        }
    }
    #[test]
    fn invalid_terminators_states_and_partial_timeout() {
        let mut parser = Parser::new();
        for byte in [0x65, 0, 3, 2, 255, 255, 255, 0x65, 0, 3, 0, 255, 254, 255] {
            assert_eq!(parser.push(byte), None);
        }
        parser.push(0x65);
        parser.push(1);
        for _ in 0..100 {
            parser.idle_tick();
        }
        for byte in [0x65, 0, 3, 0, 255, 255] {
            assert_eq!(parser.push(byte), None);
        }
        assert_eq!(
            parser.push(255),
            Some(Event::Component {
                page: 0,
                id: 3,
                pressed: false
            })
        );
    }
}
