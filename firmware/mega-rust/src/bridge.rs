// Pure byte forwarding logic, also exercised by host-side tests.
const CAPACITY: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    Receive,
    Overflow,
}

pub trait Port {
    fn read(&mut self) -> Result<Option<u8>, Fault>;
    // false means the transmit register is busy; the byte must be retained.
    fn write(&mut self, byte: u8) -> Result<bool, Fault>;
}

struct Queue {
    bytes: [u8; CAPACITY],
    head: usize,
    len: usize,
}

impl Queue {
    const fn new() -> Self {
        Self {
            bytes: [0; CAPACITY],
            head: 0,
            len: 0,
        }
    }

    fn send(&mut self, destination: &mut impl Port) -> Result<(), Fault> {
        if self.len != 0 && destination.write(self.bytes[self.head])? {
            self.head = (self.head + 1) % CAPACITY;
            self.len -= 1;
        }
        Ok(())
    }

    fn receive(&mut self, source: &mut impl Port) -> Result<(), Fault> {
        if let Some(byte) = source.read()? {
            if self.len == CAPACITY {
                return Err(Fault::Overflow);
            }
            self.bytes[(self.head + self.len) % CAPACITY] = byte;
            self.len += 1;
        }
        Ok(())
    }
}

pub struct Bridge {
    host_to_screen: Queue,
    screen_to_host: Queue,
    fault: Option<Fault>,
}

impl Bridge {
    pub const fn new() -> Self {
        Self {
            host_to_screen: Queue::new(),
            screen_to_host: Queue::new(),
            fault: None,
        }
    }

    // Bounded work: at most one transmit and receive per direction per poll.
    // A fault latches until reset; no later poll touches either serial port.
    pub fn poll(&mut self, host: &mut impl Port, screen: &mut impl Port) -> Result<(), Fault> {
        if let Some(fault) = self.fault {
            return Err(fault);
        }
        let result = (|| {
            self.host_to_screen.send(screen)?;
            self.screen_to_host.send(host)?;
            self.host_to_screen.receive(host)?;
            self.screen_to_host.receive(screen)?;
            Ok(())
        })();
        if let Err(fault) = result {
            self.fault = Some(fault);
        }
        result
    }
}

// ATmega2560 UCSRnA: RXC=7, FE=4, DOR=3, UPE=2.
// Error flags are meaningful for the next unread frame, only when RXC is set.
pub fn receive_fault(status: u8) -> bool {
    status & 0x80 != 0 && status & 0x1c != 0
}
