// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound main-thread queue dispatch independently of completion-event readiness.

/// Primitive queue operations stay with the actual Windows adapter.
pub(super) trait MessageBoundary {
    type Message;
    fn take(&mut self) -> Option<Self::Message>;
    fn quit(&self, message: &Self::Message) -> Option<i32>;
    fn dispatch(&mut self, message: Self::Message);
}
#[derive(Debug, Eq, PartialEq)]
pub(super) enum Drain {
    Empty,
    Dispatched,
    Quit(i32),
}
/// Dispatch at most 32 messages, preserving an observed quit and its remaining queue.
pub(super) fn drain(queue: &mut impl MessageBoundary) -> Drain {
    let mut result = Drain::Empty;
    for _ in 0..32 {
        let Some(message) = queue.take() else { break };
        if let Some(status) = queue.quit(&message) {
            return Drain::Quit(status);
        }
        queue.dispatch(message);
        result = Drain::Dispatched;
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    struct Queue {
        pending: VecDeque<i32>,
        dispatched: Vec<i32>,
    }
    impl MessageBoundary for Queue {
        type Message = i32;
        fn take(&mut self) -> Option<i32> {
            self.pending.pop_front()
        }
        fn quit(&self, message: &i32) -> Option<i32> {
            (*message < 0).then_some(*message)
        }
        fn dispatch(&mut self, message: i32) {
            self.dispatched.push(message);
        }
    }
    #[test]
    fn native_queue_dispatch_is_bounded_and_preserves_the_next_message() {
        let mut queue = Queue { pending: (0..33).collect(), dispatched: vec![] };
        assert_eq!(drain(&mut queue), Drain::Dispatched);
        assert_eq!(queue.dispatched, (0..32).collect::<Vec<_>>());
        assert_eq!(queue.pending, VecDeque::from([32]));
    }
    #[test]
    fn native_quit_precedes_later_dispatch_and_preserves_original_status() {
        let mut queue = Queue { pending: VecDeque::from([1, -7, 2]), dispatched: vec![] };
        assert_eq!(drain(&mut queue), Drain::Quit(-7));
        assert_eq!(queue.dispatched, [1]);
        assert_eq!(queue.pending, VecDeque::from([2]));
    }
    #[test]
    fn empty_native_queue_dispatches_nothing() {
        let mut queue = Queue { pending: VecDeque::new(), dispatched: vec![] };
        assert_eq!(drain(&mut queue), Drain::Empty);
        assert!(queue.dispatched.is_empty());
    }
}
