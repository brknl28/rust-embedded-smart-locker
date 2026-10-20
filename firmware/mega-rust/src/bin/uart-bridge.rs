#![no_std]
#![no_main]

use arduino_hal::prelude::*;
use core::convert::Infallible;
use panic_halt as _;

#[path = "../bridge.rs"]
mod bridge;
use bridge::{Bridge, Fault, Port};

struct Uart<S> {
    serial: S,
    status: fn() -> u8,
}

impl<S> Port for Uart<S>
where
    S: _embedded_hal_serial_Read<u8, Error = Infallible>
        + _embedded_hal_serial_Write<u8, Error = Infallible>,
{
    fn read(&mut self) -> Result<Option<u8>, Fault> {
        // The HAL returns Infallible and does not expose FE/DOR/UPE.
        // Snapshot status before reading UDR, which advances the receive FIFO.
        let status = (self.status)();
        if status & 0x80 == 0 {
            return Ok(None);
        }
        if bridge::receive_fault(status) {
            return Err(Fault::Receive);
        }
        match self.serial.read() {
            Ok(byte) => Ok(Some(byte)),
            Err(nb::Error::WouldBlock) => Ok(None),
            Err(nb::Error::Other(never)) => match never {},
        }
    }

    fn write(&mut self, byte: u8) -> Result<bool, Fault> {
        match self.serial.write(byte) {
            Ok(()) => Ok(true),
            Err(nb::Error::WouldBlock) => Ok(false),
            Err(nb::Error::Other(never)) => match never {},
        }
    }
}

fn host_status() -> u8 {
    // SAFETY: read-only PAC status access, serialized with the HAL operations.
    // No ISR runs; this firmware never enables interrupts or shares UARTs.
    unsafe { (*arduino_hal::pac::USART0::ptr()).ucsr0a().read().bits() }
}

fn screen_status() -> u8 {
    // SAFETY: same exclusive, single-threaded status sampling as USART0.
    unsafe { (*arduino_hal::pac::USART1::ptr()).ucsr1a().read().bits() }
}

#[arduino_hal::entry]
fn main() -> ! {
    let peripherals = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(peripherals);
    let mut fault_led = pins.d13.into_output();
    fault_led.set_low();
    let mut host = Uart {
        serial: arduino_hal::default_serial!(peripherals, pins, 9600),
        status: host_status,
    };
    let mut screen = Uart {
        serial: arduino_hal::Usart::new(
            peripherals.USART1,
            pins.d19,
            pins.d18.into_output(),
            9600.into_baudrate(),
        ),
        status: screen_status,
    };
    let mut bridge = Bridge::new();
    loop {
        if bridge.poll(&mut host, &mut screen).is_err() {
            fault_led.set_high();
            // No serial error message: reset is required after a failed stream.
            loop {
                core::hint::spin_loop();
            }
        }
    }
}
