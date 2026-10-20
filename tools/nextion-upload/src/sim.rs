use crate::protocol::{Transport, BLOCK, CHUNK, MODEL, SERIAL};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scenario {
    Success,
    MissingReady,
    MissingAck,
    BadAck,
    Disconnect,
    LateAck,
    MissingRestart,
    WrongModel,
    WrongSerial,
    WrongAddress,
}

impl Scenario {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "success" => Ok(Self::Success),
            "missing-ready" => Ok(Self::MissingReady),
            "missing-ack" => Ok(Self::MissingAck),
            "bad-ack" => Ok(Self::BadAck),
            "disconnect" => Ok(Self::Disconnect),
            "late-ack" => Ok(Self::LateAck),
            "missing-restart" => Ok(Self::MissingRestart),
            "wrong-model" => Ok(Self::WrongModel),
            "wrong-serial" => Ok(Self::WrongSerial),
            "wrong-address" => Ok(Self::WrongAddress),
            _ => Err(format!("Unknown simulation scenario: {s}")),
        }
    }
}

/// A protocol peer with a virtual clock. It is not a USB/UART electrical simulator.
pub struct Sim<'a> {
    pub received: Vec<u8>,
    pub writes: usize,
    pub upload_started: bool,
    pub max_chunk: usize,
    now: u64,
    scenario: Scenario,
    expected: &'a [u8],
    command: Vec<u8>,
    stage: u8,
    replies: VecDeque<(u64, u8)>,
}

impl<'a> Sim<'a> {
    pub fn new(expected: &'a [u8], scenario: Scenario) -> Self {
        Self {
            received: Vec::new(),
            writes: 0,
            upload_started: false,
            max_chunk: 0,
            now: 0,
            scenario,
            expected,
            command: Vec::new(),
            stage: 0,
            replies: VecDeque::new(),
        }
    }
    fn queue(&mut self, bytes: &[u8], delay: u64) {
        for &byte in bytes {
            self.replies.push_back((self.now + delay, byte));
        }
    }
}

impl Transport for Sim<'_> {
    fn now(&self) -> u64 {
        self.now
    }
    fn sleep(&mut self, ms: u64) {
        self.now += ms;
    }
    fn drain(&mut self, _deadline: u64) -> Result<(), String> {
        Ok(())
    }
    fn read(&mut self, deadline: u64) -> Result<u8, String> {
        if let Some(&(when, byte)) = self.replies.front() {
            if when <= deadline {
                self.now = self.now.max(when);
                self.replies.pop_front();
                return Ok(byte);
            }
        }
        self.now = deadline;
        Err("Receive timeout (simulated)".into())
    }
    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > CHUNK {
            return Err("Unpaced oversized write".into());
        }
        self.writes += 1;
        self.max_chunk = self.max_chunk.max(bytes.len());
        if self.stage < 2 {
            self.command.extend_from_slice(bytes);
            if !self.command.ends_with(&[0xff; 3]) {
                return Ok(());
            }
            if self.stage == 0 {
                if self.command != b"connect\xff\xff\xff" {
                    return Err("Invalid connect framing".into());
                }
                let model = if self.scenario == Scenario::WrongModel {
                    "NX8048P050_011R"
                } else {
                    MODEL
                };
                let serial = if self.scenario == Scenario::WrongSerial {
                    "OTHER"
                } else {
                    SERIAL
                };
                let address = if self.scenario == Scenario::WrongAddress {
                    12
                } else {
                    0
                };
                let mut response =
                    format!("comok 1,1793-{address},{model},1098,10501,{serial},131072000-0")
                        .into_bytes();
                response.extend_from_slice(&[0xff; 3]);
                self.queue(&response, 50);
            } else {
                let mut expected = format!("whmi-wri {},9600,0", self.expected.len()).into_bytes();
                expected.extend_from_slice(&[0xff; 3]);
                if self.command != expected {
                    return Err("Wrong upload size/baud/terminators".into());
                }
                self.upload_started = true;
                if self.scenario != Scenario::MissingReady {
                    self.queue(&[0x05], 50);
                }
            }
            self.command.clear();
            self.stage += 1;
        } else {
            if self.replies.iter().any(|(_, byte)| *byte == 0x05) {
                return Err("Payload sent before pending ACK was consumed".into());
            }
            if self.scenario == Scenario::Disconnect && self.received.len() >= BLOCK {
                return Err("Cable disconnected (simulated)".into());
            }
            let offset = self.received.len();
            if self.expected.get(offset..offset + bytes.len()) != Some(bytes) {
                return Err("Payload corruption or excess bytes".into());
            }
            self.received.extend_from_slice(bytes);
            let end = self.received.len();
            if end % BLOCK == 0 || end == self.expected.len() {
                if self.scenario == Scenario::MissingAck && end <= BLOCK {
                    return Ok(());
                }
                let ack = if self.scenario == Scenario::BadAck {
                    0x1a
                } else {
                    0x05
                };
                let delay = if self.scenario == Scenario::LateAck {
                    3500
                } else {
                    50
                };
                self.queue(&[ack], delay);
                if end == self.expected.len() && self.scenario != Scenario::MissingRestart {
                    self.queue(
                        &[0, 0, 0, 0xff, 0xff, 0xff, 0x88, 0xff, 0xff, 0xff],
                        delay + 100,
                    );
                }
            }
        }
        Ok(())
    }
}
