//! # Replacinator
//!
//! The replacinator will replace the internals of an existing string slice
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]

use core::{convert::TryInto, mem::replace};

/// A partially updated string slice
///
/// Conceptually, this is a wrapper around a region of bytes containing three consecutive segments:
/// - The first section of the memory is a valid UTF-8 string, containing exactly the
///  characters which have been 'written' to the string in order
/// - The second section of memory has arbitrary contents
/// - The third section is the characters which have not yet been read, as valid UTF-8
///
/// If this region is created from a string slice, before the borrow of that slice ends, the middle
/// section must be returned to valid UTF-8.
///
/// This is ensured by the `Drop` impl for Replacinator. However, this is not guaranteed to run
/// in an arbitrary user controlled stack - [`Replacinator::new_in`] provides a safe wrapper around this.
pub struct Replacinator<'a> {
    contents: &'a mut [u8],
    read_position: usize,
    write_position: usize,
}

impl<'a> Replacinator<'a> {
    /// Create a new [`Replacinator`] for the given string slice, and operate on it within the given function.
    /// This function can be safe because it ensures that `value` is returned to a valid string slice,
    /// by ensuring that [`Drop`] is called as required.
    pub fn new_in<R>(value: &'a mut str, mut with: impl FnMut(&mut Replacinator<'a>) -> R) -> R {
        // Safety: Because we create a new scope, `it` is always dropped,
        // so the data behind value returns to being utf8 by the end of the borrow.
        let mut it = unsafe { Self::construct(value) };
        with(&mut it)
    }

    /// Create a new [`Replacinator`] from the given string
    ///
    /// # Safety
    /// Before 'a ends, the resulting Replacinator must be synchonr
    pub unsafe fn construct(from: &'a mut str) -> Self {
        Self {
            // SAFETY: By the time this borrow ends, the memory contents are back to being utf8.
            // This is the key line of unsafety which the rest of this module ensures is kept safe
            contents: unsafe { from.as_bytes_mut() },
            read_position: 0,
            write_position: 0,
        }
    }

    /// View the string contents of the 'third section'
    pub fn remainder(&self) -> &str {
        unsafe { unchecked_from_utf8(&self.contents[self.read_position..]) }
    }

    /// View the string contents of the 'third section' mutably
    pub fn remainder_mut(&mut self) -> &mut str {
        unsafe { unchecked_from_utf8_mut(&mut self.contents[self.read_position..]) }
    }

    /// View the string contents of the first section
    pub fn start(&self) -> &str {
        unsafe { unchecked_from_utf8(&self.contents[..self.write_position]) }
    }

    /// View the string contents of the first section mutably.
    pub fn start_mut(&mut self) -> &mut str {
        unsafe { unchecked_from_utf8_mut(&mut self.contents[..self.write_position]) }
    }

    /// Take the first section as a mutable view
    pub fn take_start(&mut self) -> &'a mut str {
        let inner = &mut [];
        // Juggle the lifetimes, to avoid unneeded unsafe code
        let pre_synchronised_end = self.write_position;
        self.synchronise();
        let contents = replace(&mut self.contents, inner);
        let (start, end) = contents.split_at_mut(self.read_position);
        self.contents = end;

        self.read_position = 0;
        self.write_position = 0;
        self.check_invariants();
        unsafe { unchecked_from_utf8_mut(&mut start[..pre_synchronised_end]) }
    }

    pub fn skip_char(&mut self) -> Option<char> {
        let value = self.read_char();
        if let Some(c) = value {
            self.write_char(c)
        }
        value
    }

    pub fn peek(&self) -> Option<char> {
        self.remainder().chars().next()
    }

    pub fn read_char(&mut self) -> Option<char> {
        let value = self.remainder().chars().next();
        if let Some(c) = value {
            self.read_position += c.len_utf8();
        }
        self.check_invariants();
        value
    }

    pub fn write_char(&mut self, c: char) {
        c.encode_utf8(self.invalid_region());
        self.write_position += c.len_utf8();
        self.check_invariants();
    }

    pub fn synchronise(&mut self) {
        let bytes = self.invalid_region();
        let code: u32 = ' '.into();
        bytes.fill(code.try_into().unwrap());
        self.write_position = self.read_position;
        self.check_invariants();
    }

    fn invalid_region(&mut self) -> &mut [u8] {
        self.check_invariants();
        &mut self.contents[self.write_position..self.read_position]
    }

    /// Checks internal invariants are correct
    fn check_invariants(&self) {
        assert!(self.write_position <= self.read_position);
        if !(self.read_position <= self.contents.len()) {
            unreachable!("The read position was outside of the ");
        }
    }
}

// `'a` may not dangle, since it is invalid to use the source string
// until `'a` ends
impl<'a> Drop for Replacinator<'a> {
    fn drop(&mut self) {
        self.synchronise();
    }
}

/// Convert a byte slice into a string slice
///
/// This function uses a safe path if the safety checks are enabled:
///
/// - When debug_assertions are enabled (default in a debug build)
/// - The `"disable_safety_checks"` feature for this crate is enabled.
///
/// Note that this safe path is `O(len(v))`
///
/// ## Safety
///
/// Calling [`core::str::from_utf8_unchecked`] on the slice must be safe
unsafe fn unchecked_from_utf8(v: &[u8]) -> &str {
    #[cfg(any(debug_assertions, not(feature = "disable_safety_checks"), test))]
    {
        core::str::from_utf8(v).expect(
            "`replacinator` internally tried to create a string slice which would contain invalid UTF-8.
This indicates a soundness hole; this assertion should be unreachable.
Please report this at the issue page: https://github.com/DJMcNab/replacinator/issues.",
        )
    }

    #[cfg(not(any(debug_assertions, not(feature = "disable_safety_checks"), test)))]
    unsafe {
        // Safety: Calling this function is safe as guaranteed by the caller
        core::str::from_utf8_unchecked(v)
    }
}

/// Convert an exclusive byte slice into an exclusive string slice
///
/// The same caveats regarding the safe path used apply as in [`unchecked_from_utf8`].
///
/// ## Safety
///
/// Calling [`core::str::from_utf8_unchecked_mut`] on the input slice must be safe
unsafe fn unchecked_from_utf8_mut(v: &mut [u8]) -> &mut str {
    #[cfg(any(debug_assertions, not(feature = "disable_safety_checks"), test))]
    {
        core::str::from_utf8_mut(v).expect(
            "`replacinator` internally tried to create a string slice which would contain invalid UTF-8.
This indicates a soundness hole; this assertion should be unreachable.
Please report this at the issue page: https://github.com/DJMcNab/replacinator/issues.",
        )
    }

    #[cfg(not(any(debug_assertions, not(feature = "disable_safety_checks"), test)))]
    unsafe {
        // Safety: Calling this function is safe as guaranteed by the caller
        core::str::from_utf8_unchecked_mut(v)
    }
}
