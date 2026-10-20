// Atomic enqueue of a complete message; a full queue never emits a partial line.
pub struct ByteQueue {
    bytes: [u8; 256],
    head: usize,
    len: usize,
}

impl ByteQueue {
    pub const fn new() -> Self {
        Self {
            bytes: [0; 256],
            head: 0,
            len: 0,
        }
    }
    pub fn enqueue(&mut self, bytes: &[u8]) -> bool {
        if bytes.len() > self.bytes.len() - self.len {
            return false;
        }
        for &byte in bytes {
            self.bytes[(self.head + self.len) % self.bytes.len()] = byte;
            self.len += 1;
        }
        true
    }
    pub fn peek(&self) -> Option<u8> {
        if self.len == 0 {
            None
        } else {
            Some(self.bytes[self.head])
        }
    }
    pub fn consume(&mut self) {
        if self.len > 0 {
            self.head = (self.head + 1) % self.bytes.len();
            self.len -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ByteQueue;
    #[test]
    fn wraparound_and_atomic_overflow() {
        let mut q = ByteQueue::new();
        assert!(q.enqueue(&[1; 250]));
        assert!(!q.enqueue(&[2; 7]));
        for _ in 0..245 {
            assert_eq!(q.peek(), Some(1));
            q.consume();
        }
        assert!(q.enqueue(&[2; 20]));
        for _ in 0..5 {
            assert_eq!(q.peek(), Some(1));
            q.consume();
        }
        for _ in 0..20 {
            assert_eq!(q.peek(), Some(2));
            q.consume();
        }
        assert_eq!(q.peek(), None);
    }
}
