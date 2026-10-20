mod bridge;

use bridge::{Bridge, Fault, Port};
use std::collections::VecDeque;

#[derive(Default)]
struct MockPort {
    input: VecDeque<u8>,
    output: Vec<u8>,
    busy: bool,
    bad_receive: bool,
    calls: usize,
}

impl Port for MockPort {
    fn read(&mut self) -> Result<Option<u8>, Fault> {
        self.calls += 1;
        if self.bad_receive {
            Err(Fault::Receive)
        } else {
            Ok(self.input.pop_front())
        }
    }
    fn write(&mut self, byte: u8) -> Result<bool, Fault> {
        self.calls += 1;
        if self.busy {
            return Ok(false);
        }
        self.output.push(byte);
        Ok(true)
    }
}

#[test]
fn all_bytes_repeated_both_directions_with_busy_transmitters() {
    let payload: Vec<u8> = (0..16384).map(|i| i as u8).collect();
    let mut expected_reverse = payload.clone();
    expected_reverse.reverse();
    let mut host = MockPort {
        input: payload.iter().copied().collect(),
        ..Default::default()
    };
    let mut screen = MockPort {
        input: expected_reverse.iter().copied().collect(),
        ..Default::default()
    };
    let mut b = Bridge::new();
    // Pace synthetic arrivals like UART frames, with independent TX stalls.
    let mut host_input = std::mem::take(&mut host.input);
    let mut screen_input = std::mem::take(&mut screen.input);
    for tick in 0..200000 {
        if tick % 8 == 0 {
            if let Some(byte) = host_input.pop_front() {
                host.input.push_back(byte);
            }
            if let Some(byte) = screen_input.pop_front() {
                screen.input.push_back(byte);
            }
        }
        host.busy = tick % 5 == 0;
        screen.busy = tick % 7 == 0;
        b.poll(&mut host, &mut screen).unwrap();
    }
    assert_eq!(screen.output, payload);
    assert_eq!(host.output, expected_reverse);
}

#[test]
fn stalled_direction_does_not_block_reverse_and_retains_bytes() {
    let mut host = MockPort {
        input: vec![0, 255, 5, 10, 13].into(),
        ..Default::default()
    };
    let mut screen = MockPort {
        input: vec![255, 0, 101].into(),
        busy: true,
        ..Default::default()
    };
    let mut b = Bridge::new();
    for _ in 0..20 {
        b.poll(&mut host, &mut screen).unwrap();
    }
    assert_eq!(host.output, [255, 0, 101]);
    assert!(screen.output.is_empty());
    screen.busy = false;
    for _ in 0..20 {
        b.poll(&mut host, &mut screen).unwrap();
    }
    assert_eq!(screen.output, [0, 255, 5, 10, 13]);
}

#[test]
fn full_queue_preserves_128_bytes_and_faults_on_129th() {
    let mut host = MockPort::default();
    let mut screen = MockPort {
        busy: true,
        ..Default::default()
    };
    let mut b = Bridge::new();
    host.input = (0..128).collect();
    for _ in 0..128 {
        b.poll(&mut host, &mut screen).unwrap();
    }
    screen.busy = false;
    for _ in 0..128 {
        b.poll(&mut host, &mut screen).unwrap();
    }
    assert_eq!(screen.output, (0..128).collect::<Vec<u8>>());

    screen.busy = true;
    host.input = (0..129).collect();
    for _ in 0..128 {
        b.poll(&mut host, &mut screen).unwrap();
    }
    assert_eq!(b.poll(&mut host, &mut screen), Err(Fault::Overflow));
    let calls = host.calls + screen.calls;
    screen.busy = false;
    for _ in 0..10 {
        assert_eq!(b.poll(&mut host, &mut screen), Err(Fault::Overflow));
    }
    assert_eq!(host.calls + screen.calls, calls);
}

#[test]
fn receive_error_latches_and_reset_creates_silent_empty_bridge() {
    for fail_host in [false, true] {
        let mut host = MockPort {
            bad_receive: fail_host,
            ..Default::default()
        };
        let mut screen = MockPort {
            bad_receive: !fail_host,
            ..Default::default()
        };
        let mut b = Bridge::new();
        assert_eq!(b.poll(&mut host, &mut screen), Err(Fault::Receive));
        host.bad_receive = false;
        screen.bad_receive = false;
        assert_eq!(b.poll(&mut host, &mut screen), Err(Fault::Receive));
        let mut reset = Bridge::new();
        for _ in 0..10 {
            reset.poll(&mut host, &mut screen).unwrap();
        }
        assert!(host.output.is_empty() && screen.output.is_empty());
    }
}

#[test]
fn receive_status_checks_all_error_flags_only_for_available_frame() {
    for flag in [0x04, 0x08, 0x10] {
        assert!(bridge::receive_fault(0x80 | flag));
        assert!(!bridge::receive_fault(flag));
    }
    assert!(!bridge::receive_fault(0x80));
    assert!(!bridge::receive_fault(0x60));
}

#[test]
fn reverse_queue_overflow_also_aborts_both_directions() {
    let mut host = MockPort {
        busy: true,
        ..Default::default()
    };
    let mut screen = MockPort {
        input: (0..129).collect(),
        ..Default::default()
    };
    let mut b = Bridge::new();
    for _ in 0..128 {
        b.poll(&mut host, &mut screen).unwrap();
    }
    assert_eq!(b.poll(&mut host, &mut screen), Err(Fault::Overflow));
    let calls = host.calls + screen.calls;
    host.busy = false;
    host.input.push_back(42);
    assert_eq!(b.poll(&mut host, &mut screen), Err(Fault::Overflow));
    assert_eq!(host.calls + screen.calls, calls);
    assert!(host.output.is_empty() && screen.output.is_empty());
}
