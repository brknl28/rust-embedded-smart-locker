use super::*;
use protocol::{Transport, BLOCK, CHUNK};
use sim::{Scenario, Sim};

fn payload(len: usize) -> Vec<u8> {
    (0..len).map(|n| n as u8).collect()
}

#[test]
fn sha256_known_vectors() {
    assert_eq!(
        hash::sha256(b"").unwrap(),
        "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855"
    );
    assert_eq!(
        hash::sha256(b"abc").unwrap(),
        "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
    );
}

#[test]
fn validates_exact_size_and_hash_before_transport() {
    let data = payload(256);
    let sha = hash::sha256(&data).unwrap();
    assert!(validate(&data, 256, &sha.to_lowercase()).is_ok());
    assert!(validate(&data, 255, &sha).is_err());
    assert!(validate(&data, 256, &"0".repeat(64)).is_err());
    assert!(validate(&data, 256, "invalid").is_err());
    assert!(validate(&[], 0, &sha).is_err());
}

#[test]
fn all_byte_values_and_packet_boundaries_are_preserved() {
    for len in [1, 256, 4095, 4096, 4097, 8192, 824432] {
        let data = payload(len);
        let mut peer = Sim::new(&data, Scenario::Success);
        let report = protocol::upload(&mut peer, &data).unwrap();
        assert_eq!(peer.received, data);
        assert_eq!(report.bytes, len);
        assert_eq!(report.blocks, len.div_ceil(BLOCK));
        assert!(peer.max_chunk <= CHUNK);
        assert!(peer.upload_started);
    }
}

#[test]
fn missing_initial_ack_sends_no_payload() {
    let data = payload(8192);
    let mut peer = Sim::new(&data, Scenario::MissingReady);
    assert!(protocol::upload(&mut peer, &data)
        .unwrap_err()
        .contains("Upload ready"));
    assert!(peer.received.is_empty());
}

#[test]
fn missing_bad_or_late_block_ack_stops_without_next_block() {
    let data = payload(8192);
    for scenario in [Scenario::MissingAck, Scenario::BadAck, Scenario::LateAck] {
        let mut peer = Sim::new(&data, scenario);
        assert!(protocol::upload(&mut peer, &data)
            .unwrap_err()
            .contains("Block 1"));
        assert_eq!(peer.received.len(), BLOCK);
    }
}

#[test]
fn disconnect_stops_without_retry() {
    let data = payload(8192);
    let mut peer = Sim::new(&data, Scenario::Disconnect);
    assert!(protocol::upload(&mut peer, &data)
        .unwrap_err()
        .contains("disconnected"));
    assert_eq!(peer.received.len(), BLOCK);
}

#[test]
fn missing_restart_is_not_claimed_success() {
    let data = payload(4097);
    let mut peer = Sim::new(&data, Scenario::MissingRestart);
    assert!(protocol::upload(&mut peer, &data)
        .unwrap_err()
        .contains("readiness unconfirmed"));
    assert_eq!(peer.received, data);
}

#[test]
fn wrong_identity_or_address_prevents_upload_start() {
    let data = payload(256);
    for scenario in [
        Scenario::WrongModel,
        Scenario::WrongSerial,
        Scenario::WrongAddress,
    ] {
        let mut peer = Sim::new(&data, scenario);
        assert!(protocol::upload(&mut peer, &data).is_err());
        assert!(!peer.upload_started);
        assert!(peer.received.is_empty());
    }
}

#[test]
fn empty_payload_never_writes() {
    let mut peer = Sim::new(&[], Scenario::Success);
    assert!(protocol::upload(&mut peer, &[]).is_err());
    assert_eq!(peer.writes, 0);
}

struct MalformedPeer {
    now: u64,
    sent: Vec<u8>,
    read_count: usize,
    byte: u8,
}
impl Transport for MalformedPeer {
    fn now(&self) -> u64 {
        self.now
    }
    fn sleep(&mut self, ms: u64) {
        self.now += ms;
    }
    fn write(&mut self, b: &[u8]) -> Result<(), String> {
        self.sent.extend_from_slice(b);
        Ok(())
    }
    fn drain(&mut self, _: u64) -> Result<(), String> {
        Ok(())
    }
    fn read(&mut self, _: u64) -> Result<u8, String> {
        self.read_count += 1;
        Ok(self.byte)
    }
}

#[test]
fn unterminated_response_has_bounded_read_and_no_upload() {
    let mut peer = MalformedPeer {
        now: 0,
        sent: Vec::new(),
        read_count: 0,
        byte: b'x',
    };
    assert!(protocol::upload(&mut peer, &[1])
        .unwrap_err()
        .contains("unterminated"));
    assert_eq!(peer.read_count, 512);
    assert_eq!(peer.sent, b"connect\xff\xff\xff");
}

#[test]
fn repeated_unrelated_frames_are_bounded() {
    let mut peer = MalformedPeer {
        now: 0,
        sent: Vec::new(),
        read_count: 0,
        byte: 0xff,
    };
    assert!(protocol::upload(&mut peer, &[1])
        .unwrap_err()
        .contains("frame limit"));
    assert_eq!(peer.read_count, 24);
    assert_eq!(peer.sent, b"connect\xff\xff\xff");
}

#[test]
fn pacing_limits_chunk_size_and_virtual_line_rate() {
    let data = payload(256);
    let mut peer = MalformedPeer {
        now: 0,
        sent: Vec::new(),
        read_count: 0,
        byte: 0,
    };
    protocol::paced_send(&mut peer, &data).unwrap();
    assert_eq!(peer.sent, data);
    assert_eq!(peer.now, 16 * 27); // 16-byte wire time rounded up + 10ms.
}

#[test]
fn hardware_mode_requires_explicit_confirmation_before_file_or_port_access() {
    assert!(run(&["upload".into()])
        .unwrap_err()
        .contains("confirmation"));
    assert!(run(&[
        "simulate".into(),
        "--confirm-write".into(),
        WRITE_CONFIRMATION.into()
    ])
    .is_err());
    assert!(run(&["simulate".into(), "--baud".into(), "115200".into()]).is_err());
}
