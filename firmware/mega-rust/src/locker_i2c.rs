pub const ADDRESS: u8 = 0x12;

pub fn status_reply(response: [u8; 2]) -> Option<&'static [u8]> {
    if response[1] > 3 {
        return Some(b"ERR I2C malformed response\r\n");
    }
    match response[0] {
        0 => Some(match response[1] {
            0 => b"SIMULATED STATUS locker1=CLOSED locker2=CLOSED\r\n",
            1 => b"SIMULATED STATUS locker1=OPEN locker2=CLOSED\r\n",
            2 => b"SIMULATED STATUS locker1=CLOSED locker2=OPEN\r\n",
            _ => b"SIMULATED STATUS locker1=OPEN locker2=OPEN\r\n",
        }),
        1 => Some(b"ERR I2C invalid opcode\r\n"),
        2 => Some(b"ERR I2C invalid locker ID\r\n"),
        3 => Some(b"ERR I2C invalid request length\r\n"),
        4 => None,
        _ => Some(b"ERR I2C malformed response\r\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_response_and_all_simulated_states() {
        assert_eq!(ADDRESS, 0x12);
        assert_eq!(
            status_reply([0, 0]),
            Some(&b"SIMULATED STATUS locker1=CLOSED locker2=CLOSED\r\n"[..])
        );
        assert_eq!(
            status_reply([0, 1]),
            Some(&b"SIMULATED STATUS locker1=OPEN locker2=CLOSED\r\n"[..])
        );
        assert_eq!(
            status_reply([0, 2]),
            Some(&b"SIMULATED STATUS locker1=CLOSED locker2=OPEN\r\n"[..])
        );
        assert_eq!(
            status_reply([0, 3]),
            Some(&b"SIMULATED STATUS locker1=OPEN locker2=OPEN\r\n"[..])
        );
        assert_eq!(status_reply([4, 0]), None);
        for code in [1, 2, 3, 5, 255] {
            assert!(status_reply([code, 0]).unwrap().starts_with(b"ERR"));
        }
        assert!(status_reply([0, 4]).unwrap().starts_with(b"ERR"));
    }
}
