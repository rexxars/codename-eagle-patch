//! Owned-handle routing registry.
//!
//! Maps a handle (a heap pointer as `usize`) to an owned value, so the FFI layer
//! can tell which handles it minted (and must route) from ones it didn't. The
//! payload is generic here; the real `DecoderSession` type arrives with the
//! decoder in a later task. The global `Mutex`/`OnceLock` wrapper also lives in
//! the FFI layer — this stays a plain, unit-testable type.

use std::collections::HashMap;

/// A set of owned values keyed by their stable heap address.
///
/// FFI coupling to honour in Task 10: the handle `SmackOpen` returns to the
/// engine must be a `SmackCtx*`. So the owned session type must either carry
/// `SmackCtx` as its first `#[repr(C)]` field (making `&session as usize` equal
/// `&session.ctx as usize`), or the registry must key on the `SmackCtx` box
/// rather than the session box. Documented here; the session type isn't built
/// yet.
pub(crate) struct Registry<T> {
    entries: HashMap<usize, Box<T>>,
}

impl<T> Registry<T> {
    /// Create an empty registry.
    pub(crate) fn new() -> Registry<T> {
        Registry {
            entries: HashMap::new(),
        }
    }

    /// Box `value`, record it, and return its heap address as the handle. The
    /// value is boxed so the pointee address stays stable regardless of how the
    /// map moves the `Box` internally.
    pub(crate) fn insert(&mut self, value: T) -> usize {
        let boxed = Box::new(value);
        let handle = &*boxed as *const T as usize;
        self.entries.insert(handle, boxed);
        handle
    }

    /// Whether `handle` was minted by (and is still owned by) this registry.
    pub(crate) fn is_owned(&self, handle: usize) -> bool {
        self.entries.contains_key(&handle)
    }

    /// Remove and return the value for `handle`, if owned.
    pub(crate) fn remove(&mut self, handle: usize) -> Option<Box<T>> {
        self.entries.remove(&handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_owned_handles() {
        let mut r: Registry<u32> = Registry::new();
        let h = r.insert(7);
        assert!(r.is_owned(h));
        assert!(!r.is_owned(0xDEAD_BEEF));
        let removed = r.remove(h);
        assert_eq!(removed.as_deref(), Some(&7));
        assert!(!r.is_owned(h));
    }

    #[test]
    fn handles_are_distinct_and_stable() {
        let mut r: Registry<u32> = Registry::new();
        let a = r.insert(1);
        let b = r.insert(2);
        assert_ne!(a, b);
        assert!(r.is_owned(a) && r.is_owned(b));
        assert!(r.remove(0xDEAD_BEEF).is_none());
    }
}
