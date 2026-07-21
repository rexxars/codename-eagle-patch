//! Frame-pacing math with an injected clock (no real time source here).

/// Computes whether the engine should keep waiting before advancing past a
/// frame, so playback runs at a fixed FPS. The caller supplies the current time
/// (`now_ms`) on every call, keeping this unit-testable.
///
/// Semantics: in the engine loop `SmackWait` gates *before* drawing, and frame 0
/// shows immediately. `should_wait(now, i)` therefore holds until the *next*
/// frame's wall-clock deadline, `start + round((i + 1) * 1000 / fps)` — so each
/// displayed frame is held for `1000 / fps` ms. Returns `true` while
/// `now_ms < deadline`.
///
/// Degenerate `fps == 0.0`: the deadline offset is `+inf`, whose cast saturates
/// to `u64::MAX`, so `should_wait` is always `true` (waits forever). A real
/// caller never passes 0; this is just defined, not useful.
pub(crate) struct Pacer {
    fps: f64,
    start_ms: u64,
}

impl Pacer {
    /// Create a pacer for `fps` frames per second.
    pub(crate) fn new(fps: f64) -> Pacer {
        Pacer { fps, start_ms: 0 }
    }

    /// Record the wall-clock time (ms, from the injected clock) at which
    /// playback started.
    pub(crate) fn start(&mut self, now_ms: u64) {
        self.start_ms = now_ms;
    }

    /// True while `frame_index` should still be held on screen.
    pub(crate) fn should_wait(&self, now_ms: u64, frame_index: u32) -> bool {
        let deadline = self.start_ms + self.deadline_offset(frame_index + 1);
        now_ms < deadline
    }

    /// Rounded ms offset from `start_ms` at which `frame`'s period elapses.
    fn deadline_offset(&self, frame: u32) -> u64 {
        (f64::from(frame) * 1000.0 / self.fps).round() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_each_frame_for_one_period() {
        let mut p = Pacer::new(25.0); // 40ms per frame
        p.start(0);
        // Frame 0 is gated until frame 1's deadline (40ms).
        assert!(p.should_wait(10, 0));
        assert!(p.should_wait(39, 0));
        assert!(!p.should_wait(40, 0));
        assert!(!p.should_wait(45, 0));
        // Frame 1's deadline is 80ms.
        assert!(p.should_wait(79, 1));
        assert!(!p.should_wait(80, 1));
    }

    #[test]
    fn deadline_is_relative_to_start() {
        let mut p = Pacer::new(25.0);
        p.start(100);
        assert!(p.should_wait(139, 0));
        assert!(!p.should_wait(140, 0));
    }

    #[test]
    fn rounds_fractional_periods() {
        let mut p = Pacer::new(30.0); // 33.333ms per frame
        p.start(0);
        // Frame 0 deadline = round(1000/30) = 33ms.
        assert!(p.should_wait(32, 0));
        assert!(!p.should_wait(33, 0));
        // Frame 1 deadline = round(2000/30) = 67ms.
        assert!(p.should_wait(66, 1));
        assert!(!p.should_wait(67, 1));
    }

    #[test]
    fn zero_fps_always_waits() {
        // Degenerate: deadline offset is +inf -> u64::MAX, so always wait.
        let mut p = Pacer::new(0.0);
        p.start(0);
        assert!(p.should_wait(0, 0));
        assert!(p.should_wait(u64::MAX - 1, 0));
    }
}
