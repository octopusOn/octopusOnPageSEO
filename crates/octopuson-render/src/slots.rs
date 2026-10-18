//! Fair, cancellable backpressure. Queued URLs do not consume an execution budget.

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::Duration;

use octopuson_crawl::{CrawlControl, FetchError};

pub(crate) struct Slots {
    limit: usize,
    state: Mutex<State>,
    changed: Condvar,
}

#[derive(Default)]
struct State {
    active: usize,
    next_ticket: u64,
    waiting: VecDeque<u64>,
}

impl Slots {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            limit,
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        }
    }

    pub(crate) fn acquire(&self, control: &CrawlControl) -> Result<Guard<'_>, FetchError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let ticket = state.next_ticket;
        state.next_ticket += 1;
        state.waiting.push_back(ticket);
        loop {
            if control.is_stopped() {
                state.waiting.retain(|queued| *queued != ticket);
                self.changed.notify_all();
                return Err(FetchError::Other("rendering: rendering cancelled".into()));
            }
            if state.active < self.limit && state.waiting.front() == Some(&ticket) {
                state.waiting.pop_front();
                state.active += 1;
                self.changed.notify_all();
                return Ok(Guard(self));
            }
            state = self
                .changed
                .wait_timeout(state, Duration::from_millis(100))
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

pub(crate) struct Guard<'a>(&'a Slots);

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.active = state.active.saturating_sub(1);
        self.0.changed.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Instant;

    fn wait_for_queue(slots: &Slots, count: usize) {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if slots.state.lock().unwrap().waiting.len() == count {
                return;
            }
            assert!(Instant::now() < deadline, "worker did not join the queue");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn waiting_urls_are_admitted_in_order() {
        let slots = Slots::new(1);
        let control = CrawlControl::new();
        let occupied = slots.acquire(&control).unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::scope(|scope| {
            for order in 1..=2 {
                let (slots, control, tx) = (&slots, &control, tx.clone());
                scope.spawn(move || {
                    let _guard = slots.acquire(control).unwrap();
                    tx.send(order).unwrap();
                });
                wait_for_queue(slots, order);
            }
            drop(occupied);
            assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), 1);
            assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), 2);
        });
    }

    #[test]
    fn cancelled_waiter_leaves_queue_without_releasing_an_active_slot() {
        let slots = Slots::new(1);
        let active_control = CrawlControl::new();
        let cancelled = CrawlControl::new();
        let occupied = slots.acquire(&active_control).unwrap();
        std::thread::scope(|scope| {
            let waiter = scope.spawn(|| slots.acquire(&cancelled).is_err());
            wait_for_queue(&slots, 1);
            cancelled.stop();
            assert!(waiter.join().unwrap());
            let state = slots.state.lock().unwrap();
            assert!(state.waiting.is_empty());
            assert_eq!(state.active, 1);
        });
        drop(occupied);
        assert!(slots.acquire(&active_control).is_ok());
    }
}
