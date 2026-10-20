// Nonblocking lifecycle only: never produces a locker request.
// Tick durations are approximate loop counts, not hardware timer deadlines.
#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Probe,
    Starting,
    Online,
}

#[derive(Debug, PartialEq)]
pub enum Action {
    Probe,
    Start,
}

pub struct Link {
    state: State,
    age: u16,
    waiting: bool,
    start_requested: bool,
}

impl Link {
    pub const fn new() -> Self {
        Self {
            state: State::Probe,
            age: 1000,
            waiting: false,
            start_requested: false,
        }
    }
    pub fn online(&self) -> bool {
        self.state == State::Online
    }
    pub fn restart(&mut self) {
        *self = Self::new();
    }
    pub fn started(&mut self) {
        self.state = State::Starting;
        self.age = 0;
        self.waiting = true;
        self.start_requested = false;
    }
    pub fn page(&mut self, page: u8) {
        match self.state {
            State::Probe if self.waiting => {
                self.waiting = false;
                self.start_requested = true;
            }
            State::Starting if page == 0 => {
                self.state = State::Online;
                self.waiting = false;
                self.age = 0;
            }
            State::Online => {
                self.waiting = false;
                self.age = 0;
            }
            _ => {}
        }
    }
    pub fn tick(&mut self, tx_idle: bool) -> Option<Action> {
        self.age = self.age.saturating_add(1);
        if self.waiting && self.age >= 3000 {
            self.restart();
        }
        if !tx_idle {
            return None;
        }
        if self.start_requested {
            // Caller must enqueue application.start() and then call started().
            return Some(Action::Start);
        }
        if !self.waiting && self.age >= 1000 {
            self.age = 0;
            self.waiting = true;
            return Some(Action::Probe);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn either_power_order_requires_reply_and_main_page_before_touches() {
        let mut link = Link::new();
        assert!(!link.online());
        assert_eq!(link.tick(false), None);
        assert_eq!(link.tick(true), Some(Action::Probe));
        for _ in 0..2999 {
            assert_eq!(link.tick(true), None);
        }
        assert_eq!(link.tick(true), Some(Action::Probe));
        link.page(2); // Display may already be on any page when Mega boots.
        assert_eq!(link.tick(true), Some(Action::Start));
        link.started();
        link.page(1);
        assert!(!link.online());
        link.page(0);
        assert!(link.online());
        for _ in 0..999 {
            assert_eq!(link.tick(true), None);
        }
        assert_eq!(link.tick(true), Some(Action::Probe));
        link.page(1); // Healthy heartbeat must not reset a PIN page.
        assert!(link.online());
    }
    #[test]
    fn lost_reply_or_restart_disables_input_and_reinitializes() {
        let mut link = Link::new();
        link.started();
        link.page(0);
        for _ in 0..1000 {
            link.tick(true);
        }
        for _ in 0..3000 {
            link.tick(true);
        }
        assert!(!link.online());
        link.page(0);
        assert_eq!(link.tick(true), Some(Action::Start));
        link.started();
        link.page(0);
        link.restart();
        assert!(!link.online());
        assert_eq!(link.tick(true), Some(Action::Probe));
    }
    #[test]
    fn restart_clears_partial_pin_and_never_prepares_open() {
        use crate::application::{Application, State};
        let mut app = Application::new();
        assert_eq!(app.start().open, None);
        app.page_ready(0);
        app.component(0, 3, false);
        app.page_ready(1);
        app.component(1, 5, false);
        let update = app.start();
        assert_eq!(update.open, None);
        assert_eq!(app.state, State::Main);
        assert_eq!(app.locker, None);
        app.page_ready(0);
        app.component(0, 4, false);
        let update = app.page_ready(1);
        let mask = b"tPin.txt=\"----\"\xff\xff\xff";
        assert!(update.bytes[..update.len]
            .windows(mask.len())
            .any(|s| s == mask));
        assert_eq!(app.component(1, 18, false).open, None);
    }
    #[test]
    fn unexpected_page_detects_missed_restart_without_rejecting_inflight_reply() {
        let mut app = crate::application::Application::new();
        app.start();
        assert!(!app.unexpected_page(2)); // Navigation not acknowledged yet.
        app.page_ready(0);
        assert!(!app.unexpected_page(0));
        app.component(0, 3, false);
        assert!(!app.unexpected_page(0)); // Older heartbeat during navigation.
        app.page_ready(1);
        assert!(!app.unexpected_page(1));
        assert!(app.unexpected_page(0)); // Display reset back to main.
    }
}
