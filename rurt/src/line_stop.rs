use std::{fmt, io};

pub struct LineStopFmtWrite {
    remaining_new_lines: usize,
    pub inner: String,
}

impl LineStopFmtWrite {
    pub fn new(remaining_new_lines: usize) -> Self {
        Self {
            remaining_new_lines,
            inner: String::with_capacity(32 * remaining_new_lines),
        }
    }
}

impl fmt::Write for LineStopFmtWrite {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.remaining_new_lines = self
            .remaining_new_lines
            .saturating_sub(s.chars().filter(|&c| c == '\n').count());
        self.inner.push_str(s);
        if self.remaining_new_lines == 0 {
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}

pub struct LineStopIoWrite {
    remaining_new_lines: usize,
    pub inner: Vec<u8>,
}

impl LineStopIoWrite {
    pub fn new(remaining_new_lines: usize) -> Self {
        Self {
            remaining_new_lines,
            inner: Vec::with_capacity(32 * remaining_new_lines),
        }
    }
}

impl io::Write for LineStopIoWrite {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.remaining_new_lines = self
            .remaining_new_lines
            .saturating_sub(buf.iter().filter(|&&c| c == b'\n').count());
        self.inner.extend_from_slice(buf);
        if self.remaining_new_lines == 0 {
            Err(io::ErrorKind::BrokenPipe.into())
        } else {
            Ok(buf.len())
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
