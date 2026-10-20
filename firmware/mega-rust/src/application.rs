// IDs recorded from the actual Editor project in nextion/PHASE3B_DESIGN.md.
// Demonstration only: this PIN is public and offers no production security.
const DEMO_PIN: [u8; 4] = *b"1234";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Main,
    LockerSelected,
    PinEntry,
    AccessGranted,
    AccessDenied,
}

#[derive(Clone, Copy)]
pub struct Application {
    pub state: State,
    pub locker: Option<u8>,
    pin: [u8; 4],
    len: usize,
    waiting_page: Option<u8>,
}

pub struct Update {
    pub bytes: [u8; 256],
    pub len: usize,
    pub open: Option<[u8; 2]>,
}
impl Update {
    fn new() -> Self {
        Self {
            bytes: [0; 256],
            len: 0,
            open: None,
        }
    }
    fn command(&mut self, text: &[u8]) {
        assert!(self.len + text.len() + 3 <= self.bytes.len());
        self.bytes[self.len..self.len + text.len()].copy_from_slice(text);
        self.len += text.len();
        self.bytes[self.len..self.len + 3].fill(255);
        self.len += 3;
    }
}

impl Application {
    pub const fn new() -> Self {
        Self {
            state: State::Main,
            locker: None,
            pin: [0; 4],
            len: 0,
            waiting_page: None,
        }
    }
    fn clear_pin(&mut self) {
        self.pin.fill(0);
        self.len = 0;
    }
    // During navigation an older heartbeat reply may still be in flight.
    pub fn unexpected_page(&self, page: u8) -> bool {
        if self.waiting_page.is_some() {
            return false;
        }
        page != match self.state {
            State::Main => 0,
            State::LockerSelected | State::PinEntry => 1,
            State::AccessGranted | State::AccessDenied => 2,
        }
    }
    fn navigate(&mut self, page: u8, update: &mut Update) {
        self.waiting_page = Some(page);
        update.command(match page {
            0 => b"page pMain",
            1 => b"page pPin",
            _ => b"page pResult",
        });
        update.command(b"sendme");
    }
    pub fn start(&mut self) -> Update {
        *self = Self::new();
        let mut update = Update::new();
        self.navigate(0, &mut update);
        update
    }
    fn masked(&self, update: &mut Update) {
        update.command(match self.len {
            0 => b"tPin.txt=\"----\"",
            1 => b"tPin.txt=\"*---\"",
            2 => b"tPin.txt=\"**--\"",
            3 => b"tPin.txt=\"***-\"",
            _ => b"tPin.txt=\"****\"",
        });
    }
    pub fn page_ready(&mut self, page: u8) -> Update {
        let mut update = Update::new();
        if self.waiting_page != Some(page) {
            return update;
        }
        self.waiting_page = None;
        if page == 1 && self.state == State::LockerSelected {
            self.state = State::PinEntry;
            update.command(if self.locker == Some(1) {
                b"tLocker.txt=\"LOCKER 1\""
            } else {
                b"tLocker.txt=\"LOCKER 2\""
            });
            self.masked(&mut update);
            update.command(b"tHint.txt=\"ENTER 4 DIGITS\"");
        } else if page == 2 && matches!(self.state, State::AccessGranted | State::AccessDenied) {
            update.command(if self.locker == Some(1) {
                b"tLocker.txt=\"LOCKER 1\""
            } else {
                b"tLocker.txt=\"LOCKER 2\""
            });
            if self.state == State::AccessGranted {
                update.command(b"tResult.txt=\"ACCESS GRANTED\"");
                update.command(b"tResult.pco=11758");
                update.command(b"tDetail.txt=\"PIN OK - CHECK USB STATUS\"");
            } else {
                update.command(b"tResult.txt=\"ACCESS DENIED\"");
                update.command(b"tResult.pco=57931");
                update.command(b"tDetail.txt=\"INCORRECT PIN\"");
            }
        }
        update
    }
    pub fn component(&mut self, page: u8, id: u8, pressed: bool) -> Update {
        let mut update = Update::new();
        if pressed || self.waiting_page.is_some() {
            return update;
        }
        match (self.state, page, id) {
            (State::Main, 0, 3 | 4) => {
                self.locker = Some(id - 2);
                self.clear_pin();
                self.state = State::LockerSelected;
                self.navigate(1, &mut update);
            }
            (State::PinEntry, 1, 5..=13 | 15) => {
                if self.len < 4 {
                    self.pin[self.len] = if id == 15 { b'0' } else { b'1' + id - 5 };
                    self.len += 1;
                    self.masked(&mut update);
                    update.command(b"tHint.txt=\"ENTER 4 DIGITS\"");
                }
            }
            (State::PinEntry, 1, 14) => {
                self.clear_pin();
                self.masked(&mut update);
            }
            (State::PinEntry, 1, 16) => {
                if self.len != 0 {
                    self.len -= 1;
                    self.pin[self.len] = 0;
                }
                self.masked(&mut update);
            }
            (State::PinEntry, 1, 18) => {
                if self.len != 4 {
                    update.command(b"tHint.txt=\"ENTER ALL 4 DIGITS\"");
                } else {
                    if self.pin == DEMO_PIN {
                        self.state = State::AccessGranted;
                        update.open = self.locker.map(|locker| [1, locker]);
                    } else {
                        self.state = State::AccessDenied;
                    }
                    self.clear_pin();
                    self.navigate(2, &mut update);
                }
            }
            (State::PinEntry, 1, 17) | (State::AccessGranted | State::AccessDenied, 2, 5) => {
                update = self.start();
            }
            _ => {}
        }
        update
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_other_four_digit_pins_cannot_open_either_locker() {
        for locker in [1, 2] {
            for number in 0..10_000 {
                let pin = [
                    b'0' + (number / 1000) as u8,
                    b'0' + ((number / 100) % 10) as u8,
                    b'0' + ((number / 10) % 10) as u8,
                    b'0' + (number % 10) as u8,
                ];
                let mut app = selected(locker);
                digits(&mut app, &pin);
                assert_eq!(
                    app.component(1, 18, false).open,
                    if pin == DEMO_PIN {
                        Some([1, locker])
                    } else {
                        None
                    }
                );
            }
        }
    }
    #[test]
    fn raw_component_frames_drive_the_selected_locker() {
        use crate::hmi_event::{Event, Parser};
        let mut parser = Parser::new();
        let mut app = Application::new();
        for id in [4, 5, 6, 7, 8, 18] {
            let page = if id == 4 { 0 } else { 1 };
            let mut request = None;
            for byte in [0x65, page, id, 0, 255, 255, 255] {
                if let Some(Event::Component { page, id, pressed }) = parser.push(byte) {
                    request = app.component(page, id, pressed).open;
                }
            }
            if id == 4 {
                for byte in [0x66, 1, 255, 255, 255] {
                    if let Some(Event::Page(page)) = parser.push(byte) {
                        app.page_ready(page);
                    }
                }
            }
            assert_eq!(request, if id == 18 { Some([1, 2]) } else { None });
        }
    }
    fn selected(locker: u8) -> Application {
        let mut app = Application::new();
        app.component(0, locker + 2, false);
        assert_eq!(app.state, State::LockerSelected);
        app.page_ready(1);
        assert_eq!(app.state, State::PinEntry);
        app
    }
    fn digits(app: &mut Application, pin: &[u8]) {
        for &digit in pin {
            app.component(1, if digit == b'0' { 15 } else { digit - b'1' + 5 }, false);
        }
    }
    #[test]
    fn correct_pin_opens_only_selected_locker_once() {
        for locker in [1, 2] {
            let mut app = selected(locker);
            digits(&mut app, b"1234");
            let update = app.component(1, 18, false);
            assert_eq!(update.open, Some([1, locker]));
            assert_eq!(app.state, State::AccessGranted);
            assert_eq!(app.pin, [0; 4]);
            assert_eq!(app.component(1, 18, false).open, None);
            assert!(app
                .page_ready(2)
                .bytes
                .windows(14)
                .any(|s| s == b"ACCESS GRANTED"));
        }
    }
    #[test]
    fn wrong_and_incomplete_pin_never_open() {
        let mut app = selected(1);
        digits(&mut app, b"123");
        assert_eq!(app.component(1, 18, false).open, None);
        assert_eq!(app.state, State::PinEntry);
        digits(&mut app, b"0");
        assert_eq!(app.component(1, 18, false).open, None);
        assert_eq!(app.state, State::AccessDenied);
    }
    #[test]
    fn delete_clear_back_home_and_four_digit_limit() {
        let mut app = selected(2);
        digits(&mut app, b"1230");
        digits(&mut app, b"9");
        assert_eq!(app.len, 4);
        app.component(1, 16, false);
        digits(&mut app, b"4");
        assert_eq!(app.component(1, 18, false).open, Some([1, 2]));
        app.page_ready(2);
        app.component(2, 5, false);
        assert_eq!(app.locker, None);
        assert_eq!(app.state, State::Main);
        app.page_ready(0);
        app.component(0, 3, false);
        app.page_ready(1);
        digits(&mut app, b"12");
        app.component(1, 14, false);
        assert_eq!(app.len, 0);
        app.component(1, 16, false);
        app.component(1, 17, false);
        assert_eq!(app.pin, [0; 4]);
        assert_eq!(app.locker, None);
    }
    #[test]
    fn wrong_page_press_unknown_and_waiting_events_are_ignored() {
        let mut app = Application::new();
        assert_eq!(app.component(0, 3, true).len, 0);
        assert_eq!(app.component(1, 3, false).len, 0);
        assert_eq!(app.component(0, 99, false).len, 0);
        app.component(0, 3, false);
        assert_eq!(app.page_ready(2).len, 0);
        assert_eq!(app.component(1, 18, false).open, None);
    }
    #[test]
    fn outgoing_commands_are_terminated_and_pin_is_masked() {
        let mut app = selected(1);
        let update = app.component(1, 5, false);
        assert!(update.bytes[..update.len].ends_with(&[255; 3]));
        assert!(update.bytes[..update.len].windows(4).any(|s| s == b"*---"));
        assert!(!update.bytes[..update.len].windows(4).any(|s| s == b"1234"));
    }
}
