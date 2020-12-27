use std::io::{BufReader, IoSliceMut, Read, BufRead, Error, ErrorKind, Seek, SeekFrom};
use std::fs::File;
use std::{io, fmt, result};
use std::sync::{Mutex, Arc, MutexGuard};
use std::thread::sleep;
use std::time::Duration;


enum Maybe<T> {
    Real(T),
}

struct TailedFileRaw(BufReader<File>);
// impl Read for buffreader
pub struct TailedFile {
    inner: Arc<Mutex<BufReader<Maybe<TailedFileRaw>>>>
}

impl TailedFile {
    pub fn new(mut file: File) -> Self {
        let _ = file.seek(SeekFrom::End(0));
        let a  = BufReader::new(file);
        TailedFile {
            inner: Arc::new(Mutex::new(BufReader::with_capacity(10000, Maybe::Real(TailedFileRaw(a)))))
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
// Note that we are not calling the `.read_until` method here, but
        // rather our hardcoded implementation. For more details as to why, see
        // the comments in `read_to_end`.
        append_to_string(buf, |b| read_until(self, b'\n', b))

    }
}

fn read_until<R: BufRead + ?Sized>(r: &mut R, delim: u8, buf: &mut Vec<u8>) -> Result<usize> {
    let mut read = 0;
    loop {
        let (done, used) = {
            let available = match r.fill_buf() {
                Ok(n) if !n.is_empty() => n,
                Err(ref e) if e.kind() == ErrorKind::Interrupted => {sleep(Duration::from_millis(1)); continue;},
                Err(e) => return Err(e),
                _ => {continue}
            };
            match memchr::memchr(delim, available) {
                Some(i) => {
                    buf.extend_from_slice(&available[..=i]);
                    (true, i + 1)
                }
                None => {
                    buf.extend_from_slice(available);
                    (false, available.len())
                }
            }
        };
        r.consume(used);
        read += used;
        if done || used == 0 {
            return Ok(read);
        }
    }
}

struct Guard<'a> {
    buf: &'a mut Vec<u8>,
    len: usize,
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        unsafe {
            self.buf.set_len(self.len);
        }
    }
}


pub type Result<T> = result::Result<T, Error>;
fn append_to_string<F>(buf: &mut String, f: F) -> Result<usize>
    where
        F: FnOnce(&mut Vec<u8>) -> Result<usize>,
{
    unsafe {
        let mut g = Guard { len: buf.len(), buf: buf.as_mut_vec() };
        let ret = f(g.buf);
        if std::str::from_utf8(&g.buf[g.len..]).is_err() {
            ret.and_then(|_| {
                Err(Error::new(ErrorKind::InvalidData, "stream did not contain valid UTF-8"))
            })
        } else {
            g.len = g.buf.len();
            ret
        }
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
        }
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        match self {
            Maybe::Real(r) => handle_ebadf(r.read_vectored(bufs), 0),
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