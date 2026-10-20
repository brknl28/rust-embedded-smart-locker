#![no_std]
#![no_main]

use arduino_hal::prelude::*;
use panic_halt as _;

mod application;
mod command;
mod hmi_event;
mod hmi_link;
mod locker_i2c;
mod nextion;
mod queue;

#[arduino_hal::entry]
fn main() -> ! {
    let peripherals = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(peripherals);
    let mut led = pins.d13.into_output();
    let mut bus = arduino_hal::I2c::new(
        peripherals.TWI,
        pins.d20.into_pull_up_input(),
        pins.d21.into_pull_up_input(),
        50_000,
    );
    // Write with STOP, then poll later so Uno's main loop can execute the request.
    let mut locker_pending: Option<([u8; 2], u16)> = None;
    // USART0: RX D0 / TX D1, connected to the board's USB-to-UART bridge.
    let mut serial = arduino_hal::default_serial!(peripherals, pins, 9600);
    let mut screen = arduino_hal::Usart::new(
        peripherals.USART1,
        pins.d19,
        pins.d18.into_output(),
        9600.into_baudrate(),
    );
    let mut command = command::CommandBuffer::new();
    let mut touch = hmi_event::Parser::new();
    let mut application = application::Application::new();
    let mut hmi_link = hmi_link::Link::new();
    let mut hmi_tx = queue::ByteQueue::new();
    let mut output = queue::ByteQueue::new();
    // One outstanding screen command; its USB reply follows UART transmission.
    let mut pending: Option<(&'static [u8], &'static [u8], usize)> = None;
    let mut led_ticks: u16 = 0;

    output.enqueue(b"Mega Rust ready\r\n");

    loop {
        // Bound the work per iteration so incoming traffic cannot starve the LED.
        for _ in 0..16 {
            let byte = match serial.read() {
                Ok(byte) => byte,
                Err(nb::Error::WouldBlock) => break,
                Err(nb::Error::Other(_)) => unreachable!(),
            };
            if let Some(action) = command.push(byte) {
                match action {
                    command::Action::HmiStart => {
                        if pending.is_none() && locker_pending.is_none() && hmi_tx.peek().is_none()
                        {
                            let update = application.start();
                            hmi_tx.enqueue(&update.bytes[..update.len]);
                            hmi_link.started();
                            output.enqueue(b"HMI application enabled\r\n");
                        } else {
                            output.enqueue(b"ERR HMI busy\r\n");
                        }
                    }
                    command::Action::Locker(request) => {
                        if locker_pending.is_none() {
                            locker_pending = Some((request, 0));
                        } else {
                            output.enqueue(b"ERR I2C busy\r\n");
                        }
                    }
                    command::Action::Reply(reply) => {
                        output.enqueue(reply);
                    }
                    command::Action::Nextion { bytes, reply } => {
                        if pending.is_none() && hmi_tx.peek().is_none() {
                            pending = Some((bytes, reply, 0));
                        } else {
                            output.enqueue(b"ERR Nextion busy\r\n");
                        }
                    }
                }
            }
        }

        if let Some((request, age)) = locker_pending {
            if age == 0 {
                if _embedded_hal_blocking_i2c_Write::write(&mut bus, locker_i2c::ADDRESS, &request)
                    .is_err()
                {
                    output.enqueue(b"ERR I2C write failed\r\n");
                    locker_pending = None;
                } else {
                    locker_pending = Some((request, 1));
                }
            } else if age >= 250 {
                output.enqueue(b"ERR I2C response timeout; state unknown\r\n");
                locker_pending = None;
            } else {
                locker_pending = Some((request, age + 1));
                if age % 10 == 0 {
                    let mut response = [0; 2];
                    if _embedded_hal_blocking_i2c_Read::read(
                        &mut bus,
                        locker_i2c::ADDRESS,
                        &mut response,
                    )
                    .is_err()
                    {
                        output.enqueue(b"ERR I2C read failed; state unknown\r\n");
                        locker_pending = None;
                    } else if let Some(reply) = locker_i2c::status_reply(response) {
                        output.enqueue(reply);
                        locker_pending = None;
                    }
                }
            }
        }

        for _ in 0..16 {
            let byte = match screen.read() {
                Ok(byte) => byte,
                Err(nb::Error::WouldBlock) => break,
                Err(nb::Error::Other(_)) => unreachable!(),
            };
            if let Some(event) = touch.push(byte) {
                match event {
                    hmi_event::Event::Ready => {
                        hmi_link.restart();
                        application = application::Application::new();
                    }
                    hmi_event::Event::Touch(event) => {
                        let line = event.line();
                        output.enqueue(&line.bytes[..line.len]);
                    }
                    event => {
                        if let hmi_event::Event::Page(page) = event {
                            if hmi_link.online() && application.unexpected_page(page) {
                                hmi_link.restart();
                                application = application::Application::new();
                            }
                            hmi_link.page(page);
                        }
                        if !hmi_link.online() {
                            continue;
                        }
                        if pending.is_none()
                            && (locker_pending.is_none()
                                || matches!(event, hmi_event::Event::Page(_)))
                            && hmi_tx.peek().is_none()
                        {
                            let mut candidate = application;
                            let update = match event {
                                hmi_event::Event::Component { page, id, pressed } => {
                                    candidate.component(page, id, pressed)
                                }
                                hmi_event::Event::Page(page) => candidate.page_ready(page),
                                _ => unreachable!(),
                            };
                            if hmi_tx.enqueue(&update.bytes[..update.len]) {
                                application = candidate;
                                if let Some(request) = update.open {
                                    locker_pending = Some((request, 0));
                                }
                            }
                        } else {
                            output.enqueue(b"ERR HMI busy; repeat touch\r\n");
                        }
                    }
                }
            }
        }

        let was_online = hmi_link.online();
        let action =
            hmi_link.tick(pending.is_none() && locker_pending.is_none() && hmi_tx.peek().is_none());
        if was_online && !hmi_link.online() {
            application = application::Application::new();
        }
        match action {
            Some(hmi_link::Action::Probe) => {
                // Flush any incomplete command left by a display reset first.
                hmi_tx.enqueue(b"\xff\xff\xffsendme\xff\xff\xff");
            }
            Some(hmi_link::Action::Start) => {
                let update = application.start();
                if hmi_tx.enqueue(&update.bytes[..update.len]) {
                    hmi_link.started();
                    output.enqueue(b"HMI automatic start\r\n");
                }
            }
            None => {}
        }

        // HMI commands are complete, FF-terminated messages; never mix DIM bytes.
        if pending.is_none() {
            for _ in 0..8 {
                let Some(byte) = hmi_tx.peek() else {
                    break;
                };
                match screen.write(byte) {
                    Ok(()) => hmi_tx.consume(),
                    Err(nb::Error::WouldBlock) => break,
                    Err(nb::Error::Other(_)) => unreachable!(),
                }
            }
        }

        if let Some((bytes, reply, mut position)) = pending {
            // Do not wait for a free transmit register or for a Nextion reply.
            if position < bytes.len() {
                match screen.write(bytes[position]) {
                    Ok(()) => position += 1,
                    Err(nb::Error::WouldBlock) => {}
                    Err(nb::Error::Other(_)) => unreachable!(),
                }
            }
            // The trait flush is nonblocking; HAL's inherent flush would block.
            if position == bytes.len() && _embedded_hal_serial_Write::flush(&mut screen).is_ok() {
                output.enqueue(reply);
                pending = None;
            } else {
                pending = Some((bytes, reply, position));
            }
        }

        for _ in 0..16 {
            let Some(byte) = output.peek() else {
                break;
            };
            match serial.write(byte) {
                Ok(()) => output.consume(),
                Err(nb::Error::WouldBlock) => break,
                Err(nb::Error::Other(_)) => unreachable!(),
            }
        }

        arduino_hal::delay_ms(1);
        touch.idle_tick();
        led_ticks += 1;
        if led_ticks == 500 {
            led.toggle();
            led_ticks = 0;
        }
    }
}
