use std::io::{BufReader, IoSliceMut, Read, BufRead, Error, ErrorKind};
use std::fs::File;
use std::{io, fmt};
use std::sync::{Mutex, Arc, MutexGuard};

enum Maybe<T> {
    Real(T),
    Fake,
}

struct TailedFileRaw(BufReader<File>);
// impl Read for buffreader
pub struct TailedFile {
    inner: Arc<Mutex<BufReader<Maybe<TailedFileRaw>>>>
}

impl TailedFile {
    pub fn new(file: File) -> Self {
        TailedFile {
            inner: Arc::new(Mutex::new(BufReader::with_capacity(10000, Maybe::Real(TailedFileRaw(BufReader::new(file))))))
        }

    }
    pub fn lock(&self) -> TailedFileLock<'_> {
        TailedFileLock { inner: self.inner.lock().unwrap_or_else(|e| e.into_inner()) }
    }

    /*
    pub fn lock(&self) -> StdinLock<'_> {
        StdinLock { inner: self.inner.lock().unwrap_or_else(|e| e.into_inner()) }
    }
     */
}

pub struct TailedFileLock<'a> {
    inner: MutexGuard<'a, BufReader<Maybe<TailedFileRaw>>>,
}

impl fmt::Debug for TailedFileLock<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("TailedFileLock { .. }")
    }
}

impl BufRead for TailedFileLock<'_> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.inner.fill_buf()
    }

    fn consume(&mut self, n: usize) {
        self.inner.consume(n)
    }

    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.inner.read_until(byte, buf)
    }

    fn read_line(&mut self, buf: &mut String) -> io::Result<usize> {

        let res = self.inner.read_line(buf);
        let size : usize = 0;
        if res.is_ok() && res.as_ref().unwrap() == &size {
            return io::Result::Err(Error::new(ErrorKind::Interrupted, ""))
        }

        res
    }
}

impl Read for TailedFileLock<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.inner.read_vectored(bufs)
    }

    // #[inline]
    // fn is_read_vectored(&self) -> bool {
    //     self.inner.is_read_vectored()
    // }



    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.inner.read_to_end(buf)
    }

    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        self.inner.read_to_string(buf)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.inner.read_exact(buf)
    }
}

impl Read for TailedFileRaw {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.0.read_vectored(bufs)
    }

    // #[inline]
    // fn is_read_vectored(&self) -> bool {
    //     self.0.is_read_vectored()
    // }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.0.read_to_end(buf)
    }

    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        self.0.read_to_string(buf)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.0.read_exact(buf)
    }
}

impl<R: io::Read> io::Read for Maybe<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match *self {
            Maybe::Real(ref mut r) => handle_ebadf(r.read(buf), 0),
            Maybe::Fake => Ok(0),
        }
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        match self {
            Maybe::Real(r) => handle_ebadf(r.read_vectored(bufs), 0),
            Maybe::Fake => Ok(0),
        }
    }

}

fn handle_ebadf<T>(r: io::Result<T>, default: T) -> io::Result<T> {

    match r {
        // Err(ref e) if stdio::is_ebadf(e) => Ok(default),
        Err(ref _e) => Ok(default),
        r => r,
    }
}