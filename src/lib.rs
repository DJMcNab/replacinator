#![no_std]

use core::{convert::TryInto, marker::PhantomData, slice, str::from_utf8, usize};

pub struct Replacinator<'a> {
    // Should be NonNull<[u8]>, however the convencience methods for that aren't stable
    start: *mut u8,
    len: usize,
    read_position: usize,
    write_position: usize,

    // Keep the lifetime as a mutable borrow of the string
    phantom: PhantomData<&'a mut str>,
}

impl<'a> Replacinator<'a> {
    pub fn new_in<R>(value: &'a mut str, mut with: impl FnMut(&mut Replacinator<'a>) -> R) -> R {
        let mut it = unsafe { Self::construct(value) };
        with(&mut it)
    }
    pub unsafe fn construct(from: &'a mut str) -> Self {
        let bytes = from.as_bytes_mut();
        Self {
            start: bytes.as_mut_ptr(),
            len: bytes.len(),
            read_position: 0,
            write_position: 0,
            phantom: PhantomData,
        }
    }

    pub fn remainder(&self) -> &str {
        unsafe {
            core::str::from_utf8_unchecked(slice::from_raw_parts(
                self.start.add(self.read_position),
                self.len - self.read_position,
            ))
        }
    }

    pub fn get_begin(&mut self) -> &'a mut str {
        let res = unsafe {
            core::str::from_utf8_unchecked_mut(slice::from_raw_parts_mut(
                self.start,
                self.write_position,
            ))
        };

        self.start = unsafe { self.start.add(self.write_position) };
        self.read_position -= self.write_position;
        self.len -= self.write_position;
        self.write_position = 0;
        res
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
        value
    }

    pub fn write_char(&mut self, c: char) {
        let slice = unsafe {
            slice::from_raw_parts_mut(
                self.start.add(self.write_position),
                // Can only write in the area between the two 'pointers'
                self.read_position - self.write_position,
            )
        };
        c.encode_utf8(slice);
        self.write_position += c.len_utf8();
    }

    pub fn synchronise(&mut self) {
        let bytes = self.no_mans_land();
        let code: u32 = ' '.into();
        bytes.fill(code.try_into().unwrap());
        self.write_position = self.read_position;
    }

    pub fn no_mans_land(&mut self) -> &mut [u8] {
        unsafe {
            slice::from_raw_parts_mut(
                self.start.add(self.write_position),
                self.read_position - self.write_position,
            )
        }
    }
}

impl<'a> Drop for Replacinator<'a> {
    fn drop(&mut self) {
        self.synchronise();
        let slice = unsafe { core::slice::from_raw_parts(self.start, self.len) };
        // Assert that we still have correct utf-8.
        // In future, once this code is more confident
        let _ = from_utf8(slice).unwrap();
    }
}
