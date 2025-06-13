#[warn(unused_imports)]
use std::cell::Cell;
use std::fmt::{Display, Formatter};
use std::thread;
use std::mem::transmute;
use std::io::{Error as CErr, ErrorKind, Read, Write};
use std::rc::Rc;
use std::time::{Duration, Instant};
use libc::{c_int, close, connect, fcntl, read, send, sockaddr_un, socket, AF_UNIX, F_GETFL, F_SETFL, O_NONBLOCK, SOCK_STREAM};
use crate::serializable::Serializable;

#[macro_export]
macro_rules! sock_write {
    ($sock:expr; _:$len:expr, $($rest:tt)*) =>{
        $sock.clear_write_buf();
        $sock.write_discard_bytes($len);
        sock_write!(@noclear $sock; $($rest)*);
    };

    ($sock:expr; $varname:ident, $($rest:tt)*) =>{
        $sock.clear_write_buf();
        $sock.write_serializable($varname)
        sock_write!(@noclear $sock; $($rest)*);
    };

    ($sock:expr; _:$len:expr) => {
        $sock.clear_write_buf();
        $sock.write_discard_bytes($len);
    };

    ($sock:expr; $varname:ident) => {
        $sock.clear_write_buf();
        $sock.write_serializable($varname)?;
    };

    (@noclear $sock:expr; _:$len:expr, $($rest:tt)*) =>{
        $sock.write_discard_bytes($len);
        sock_write!(@noclear $sock; $($rest)*);
    };

    (@noclear $sock:expr; $varname:ident, $($rest:tt)*) =>{
        $sock.write_serializable($varname)
        sock_write!(@noclear $sock; $($rest)*);
    };

    (@noclear $sock:expr; _:$len:expr) => {
        $sock.write_discard_bytes($len);
    };

    (@noclear $sock:expr; $varname:ident) => {
        $sock.write_serializable($varname)?;
    };
}

#[macro_export]
macro_rules! sock_read {
    ($sock:expr; _:$len:expr, $($rest:tt)*) =>{
        $sock.clear_read_buf();
        $sock.read_discard_bytes($len)?;
        sock_read!(@noclear $sock; $($rest)*);
    };

    ($sock:expr; $varname:ident[$t:ty:$len:expr], $($rest:tt)*) =>{
        $sock.clear_read_buf();
        let $varname = $sock.read_serializable::<$t>($len)?;
        sock_read!(@noclear $sock; $($rest)*);
    };

    ($sock:expr; _:$len:expr) =>{
        $sock.clear_read_buf();
        $sock.read_discard_bytes($len)?;
    };

    ($sock:expr; $varname:ident[$t:ty:$len:expr]) =>{
        $sock.clear_read_buf();
        let $varname = $sock.read_serializable::<$t>($len)?;
    };

    (@noclear $sock:expr; _:$len:expr, $($rest:tt)*) =>{
        $sock.read_discard_bytes($len)?;
        sock_read!(@noclear $sock; $($rest)*);
    };

    (@noclear $sock:expr; $varname:ident[$t:ty:$len:expr], $($rest:tt)*) =>{
        let $varname = $sock.read_serializable::<$t>($len)?;
        sock_read!(@noclear $sock; $($rest)*);
    };

    (@noclear $sock:expr; _:$len:expr) =>{
        $sock.read_discard_bytes($len)?;
    };

    (@noclear $sock:expr; $varname:ident[$t:ty:$len:expr]) =>{
        let $varname = $sock.read_serializable::<$t>($len)?;
    };
}

pub struct Socket{
    socket_file_descriptor: Rc<Cell<Option<i32>>>,
    write_buf: Vec<u8>,
    read_buf: Vec<u8>,
    retry: bool,
    retry_frequency: u32,
    retry_timeout: u32
}

#[allow(dead_code)]
impl Socket{
    pub fn set_retry(&mut self, retry: bool){
        self.retry = retry;
    }

    pub fn set_retry_frequency(&mut self, frequency: u32){
        self.retry_frequency = frequency;
    }

    pub fn set_retry_timeout(&mut self, timeout: u32){
        self.retry_timeout = timeout;
    }

    pub fn clear_read_buf(&mut self){
        self.read_buf.clear();
    }

    pub fn clear_write_buf(&mut self){
        self.write_buf.clear();
    }

    pub fn len_read_buf(&self) -> usize{
        self.read_buf.len()
    }

    pub fn len_write_buf(&self) -> usize{
        self.write_buf.len()
    }

    pub fn write_pad(&mut self, mod_bytes: usize) -> Result<usize, SockError>{
        let len = (mod_bytes - (self.len_write_buf() % mod_bytes)) % mod_bytes;
        self.write_all(vec![0; len])?;
        Ok(len)
    }

    pub fn write_discard_bytes(&mut self, len: usize) -> Result<(), SockError>{
        self.write_all(vec![0; len])
    }

    pub fn write_serializable<T>(&mut self, to_write: T) -> Result<(), SockError> where T: Serializable{
        self.write_all(to_write.bytes())?;
        Ok(())
    }

    pub fn write_all(&mut self, data: Vec<u8>) -> Result<(), SockError> {
        let bytes_written = self.write(data.as_slice())?;
        if bytes_written != data.len() {
            Err(SockError::IncompleteWrite)
        }else {
            Ok(())
        }
    }

    pub fn flush_all(&mut self) -> Result<(), SockError> {
        self.flush()?;
        Ok(())
    }

    pub fn read_serializable<T>(&mut self, num_bytes: usize) -> Result<T, SockError> where T: Serializable{
        Ok(T::from_bytes(&self.read_bytes(num_bytes)?))
    }

    pub fn read_pad(&mut self, mod_bytes: usize) -> Result<usize, SockError>{
        let len = self.len_read_buf();
        self.clear_read_buf();
        let ret = (mod_bytes - (len % mod_bytes)) % mod_bytes;
        self.read_discard_bytes(ret)?;
        Ok(ret)
    }

    pub fn read_discard_bytes(&mut self, num_bytes: usize) -> Result<(), SockError> {
        self.read_bytes(num_bytes)?;
        Ok(())
    }

    pub fn read_bytes(&mut self, num_bytes: usize) -> Result<Vec<u8>, SockError> {
        let mut buf = vec![0u8; num_bytes];
        let bytes_read = self.read(&mut buf)?;
        if bytes_read == num_bytes {
            Ok(buf)
        }else{
            Err(SockError::IncompleteRead)
        }
    }

    pub fn read_bytes_raw<const N: usize>(&mut self) -> Result<[u8; N], SockError> {
        let mut ret: [u8; N] = [0; N];
        let bytes_read = self.read(&mut ret)?;
        if bytes_read == N {
            Ok(ret)
        }else{
            Err(SockError::IncompleteRead)
        }
    }

    pub fn read_all(&mut self) -> Result<Vec<u8>, SockError> {
        let mut pre: Vec<u8> = vec![0; 16384];
        let previous_retry = self.retry;
        self.retry = false;
        let my_read = self.read(pre.as_mut_slice());
        self.retry = previous_retry;
        match my_read{
            Ok(bytes_read) => {
                if bytes_read < pre.len() - 1{
                    Ok(Vec::from(pre[0..bytes_read].to_vec()))
                }else{
                    let next_read_all = self.read_all()?;
                    pre.extend(next_read_all);
                    Ok(pre)
                }
            }
            Err(e) => {println!("Error reading: {}", e); Err(SockError::from(e))}
        }
    }

    pub fn new(path: &str) -> Result<Self, SockError> {
        let mut sock_addr: sockaddr_un = sockaddr_un{ sun_family: 0, sun_path: [0; 108] };
        if path.len() >= sock_addr.sun_path.len() {
            return Err(SockError::PathTooLong);
        }

        sock_addr.sun_family = AF_UNIX as u16;

        let sun_path_slice = &mut sock_addr.sun_path[0..path.len()];
        let path_bytes = path.as_bytes();
        for i in 0..path.len() {
            sun_path_slice[i] = path_bytes[i] as i8;
        }

        let file_descriptor: i32;
        unsafe {
            file_descriptor = socket(AF_UNIX, SOCK_STREAM, 0);

            if file_descriptor == -1{
                return Err(SockError::InitializeError(CErr::last_os_error().raw_os_error().unwrap()));
            }

            let connect_ret = connect(file_descriptor, transmute::<&sockaddr_un, *const _>(&sock_addr), size_of::<sockaddr_un>() as u32);
            if connect_ret == -1{
                return Err(SockError::ConnectError(CErr::last_os_error().raw_os_error().unwrap()));
            }
        }

        Ok(Self {socket_file_descriptor: Rc::new(Cell::new(Some(file_descriptor))), write_buf: Vec::new(), read_buf: Vec::new(), retry: true, retry_frequency: 0, retry_timeout: 10000000})
    }
}

impl Read for Socket{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, CErr> {
        let socket_fd = match self.socket_file_descriptor.get(){
            None => return Err(CErr::new(ErrorKind::InvalidInput, "Socket file descriptor not present.")),
            Some(fd) => fd
        };
        let mut start_time = Instant::now();
        let mut c_result: i32;
        let mut read_buf: Vec<u8> = Vec::new();
        let mut done = false;

        unsafe{
            let flags = fcntl(socket_fd.clone(), F_GETFL, 0);
            if (flags & O_NONBLOCK as c_int) == 0 {
                fcntl(socket_fd.clone(), F_SETFL, flags | O_NONBLOCK);
            }
        }

        while !done {
            c_result = unsafe{ read(socket_fd.clone(), buf.as_mut_ptr() as _, buf.len()) as i32 };

            if c_result > 0{
                start_time = Instant::now();

                for i in 0..c_result{
                    read_buf.push(buf[i as usize]);
                }
            }

            if read_buf.len() as i32 != buf.len() as i32 {
                if self.retry && self.retry_timeout >= (start_time.elapsed().as_nanos() as u32){
                    thread::sleep(Duration::from_nanos(self.retry_frequency as u64));
                    continue;
                }else{
                    if self.retry {
                        return Err(CErr::new(ErrorKind::TimedOut, "Socket read timeout."));
                    }

                    done = true;
                }
            } else {
                done = true;
            }
        }

        let len = read_buf.len();
        self.read_buf.extend(read_buf);
        Ok(len)
    }
}

impl Write for Socket{
    fn write(&mut self, buf: &[u8]) -> Result<usize, CErr> {
        let original_len = self.write_buf.len();
        match self.write_buf.write_all(buf){
            Ok(()) => Ok(self.write_buf.len() - original_len),
            Err(e) => Err(CErr::new(ErrorKind::OutOfMemory, e))
        }
    }

    fn flush(&mut self) -> Result<(), CErr> {
        let c_result: i32;
        unsafe {
            match self.socket_file_descriptor.get(){
                None => return Err(CErr::new(ErrorKind::InvalidData, "Socket file descriptor not present.")),
                Some(socket_fd) => c_result = send(socket_fd.clone(), self.write_buf.as_ptr() as _, self.write_buf.len(), 0) as i32
            }
        }

        if c_result != self.write_buf.len() as i32{
            return Err(CErr::new(ErrorKind::InvalidData, "WriteError"));
        }

        self.write_buf.clear();

        Ok(())
    }
}

impl Clone for Socket{
    fn clone(&self) -> Self {
        Socket{
            socket_file_descriptor: Rc::clone(&self.socket_file_descriptor),
            write_buf: Vec::new(),
            read_buf: Vec::new(),
            retry: self.retry,
            retry_frequency: self.retry_frequency,
            retry_timeout: self.retry_timeout
        }
    }
}
impl Drop for Socket {
    fn drop(&mut self) {
        unsafe {
            match self.socket_file_descriptor.get() {
                None => println!("Socket file descriptor already dropped."),
                Some(socket_fd) => {
                    if Rc::strong_count(&self.socket_file_descriptor) == 1 {
                        let close_ret = close(socket_fd);
                        if close_ret == -1 {
                            //The close documentation suggests it should not be retried after an error.
                            println!("Socket close on drop encountered error: {}, sock_fd: {}", CErr::last_os_error().raw_os_error().unwrap(), socket_fd);
                        }
                    }
                }
            }

            self.socket_file_descriptor.replace(None);
        }
    }
}

#[derive(Debug)]
pub enum SockError{
    InitializeError(i32),
    PathTooLong,
    ConnectError(i32),
    SendError(i32),
    SendIncomplete,
    RecvError(i32),
    BufError,
    TimedOutRead,
    IncompleteRead,
    IncompleteWrite,
    UnknownError
}

impl Display for SockError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "SockError[{:?}]", self)
    }
}

impl From<CErr> for SockError {
    fn from(e: CErr) -> Self {
        match e.kind() {
            ErrorKind::InvalidInput => SockError::RecvError(CErr::last_os_error().raw_os_error().unwrap()),
            ErrorKind::InvalidData => SockError::SendError(CErr::last_os_error().raw_os_error().unwrap()),
            ErrorKind::OutOfMemory => SockError::BufError,
            ErrorKind::TimedOut => SockError::TimedOutRead,
            _ => SockError::UnknownError
        }
    }
}