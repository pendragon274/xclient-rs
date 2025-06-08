use std::cell::Cell;
use std::error::Error;
#[warn(unused_imports)]
use std::fmt::{Display, Formatter};
use std::mem;
use std::mem::transmute;
use std::io::{BufReader, BufWriter, Error as CErr, ErrorKind, Read, Write};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use libc::{c_int, close, connect, read, send, sockaddr_un, socket, AF_UNIX, SOCK_STREAM};

#[derive(Debug)]
pub enum SockError{
    InitializeError(i32),
    PathTooLong,
    ConnectError(i32),
    SendError(i32),
    SendIncomplete,
    RecvError(i32),
    BufError,
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
            _ => SockError::UnknownError
        }
    }
}

pub struct Socket{
    socket_file_descriptor: Rc<Cell<Option<i32>>>,
    buf: Vec<u8>
}

impl Socket{
    pub fn clear_buf(&mut self){
        self.buf.clear();
    }

    pub fn buf_len(&self) -> usize{
        self.buf.len()
    }

    pub fn write_all(&mut self, data: Vec<u8>) -> Result<(), SockError> {
        match self.write(data.as_slice()){
            Ok(_) => Ok(()),
            Err(e) => Err(SockError::from(e))
        }
    }

    pub fn flush_all(&mut self) -> Result<(), SockError> {
        match self.flush(){
            Ok(_) => Ok(()),
            Err(e) => Err(SockError::from(e))
        }
    }

    pub fn read_u8(&mut self) -> Result<u8, SockError> {
        Ok(self.read_bytes(1)?[0])
    }

    pub fn read_discard_bytes(&mut self, num_bytes: usize) -> Result<(), SockError> {
        match self.read_bytes(num_bytes){
            Ok(_) => Ok(()),
            Err(e) => Err(SockError::from(e))
        }
    }

    pub fn read_bytes(&mut self, num_bytes: usize) -> Result<Vec<u8>, SockError> {
        let mut buf = vec![0u8; num_bytes];
        match self.read(&mut buf) {
            Ok(u) => {
                if u == num_bytes {
                    Ok(buf[0..num_bytes].to_vec())
                }else{
                    Err(SockError::RecvError(0))
                }
            }
            Err(e) => Err(SockError::from(e))
        }
    }

    pub fn read_all(&mut self) -> Result<Vec<u8>, SockError> {
        let mut pre: Vec<u8> = vec![0; 16384];
        match self.read(pre.as_mut_slice()) {
            Ok(bytes_read) => {
                if bytes_read < pre.len() - 1{
                    Ok(Vec::from(pre[0..bytes_read].to_vec()))
                }else{
                    let next_read_all = self.read_all();
                    match next_read_all {
                        Ok(next) => {
                            pre.extend(next);
                            Ok(pre)
                        },
                        Err(e) => Err(SockError::from(e))
                    }
                }
            }
            Err(e) => Err(SockError::from(e))
        }
    }

    pub fn new(path: &str) -> Result<Self, SockError> {
        println!("Connecting to socket at {}", path);
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
            println!("Socket");
            file_descriptor = socket(AF_UNIX, SOCK_STREAM, 0);

            if file_descriptor == -1{
                return Err(SockError::InitializeError(CErr::last_os_error().raw_os_error().unwrap()));
            }

            println!("Connect");
            let connect_ret = connect(file_descriptor, transmute::<&sockaddr_un, *const _>(&sock_addr), mem::size_of::<sockaddr_un>() as u32);
            if connect_ret == -1{
                return Err(SockError::ConnectError(CErr::last_os_error().raw_os_error().unwrap()));
            }
            println!("Done connecting");
        }

        Ok(Self {socket_file_descriptor: Rc::new(Cell::new(Some(file_descriptor))), buf: Vec::new()})
    }
}

impl Read for Socket{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, CErr> {
        let c_result: i32;
        unsafe {
            match self.socket_file_descriptor.get(){
                None => return Err(CErr::new(ErrorKind::InvalidInput, "Socket file descriptor not present.")),
                Some(socket_fd) => c_result = read(socket_fd.clone(), buf.as_mut_ptr() as _, buf.len()) as i32
            }
        }

        if c_result == -1{
            return Err(CErr::new(ErrorKind::InvalidInput, "ReadError"));
        }

        Ok(c_result as usize)
    }
}

impl Write for Socket{
    fn write(&mut self, buf: &[u8]) -> Result<usize, CErr> {
        let original_len = self.buf.len();
        match self.buf.write_all(buf){
            Ok(()) => Ok(self.buf.len() - original_len),
            Err(e) => Err(CErr::new(ErrorKind::OutOfMemory, e))
        }
    }

    fn flush(&mut self) -> Result<(), CErr> {
        let c_result: i32;
        unsafe {
            match self.socket_file_descriptor.get(){
                None => return Err(CErr::new(ErrorKind::InvalidData, "Socket file descriptor not present.")),
                Some(socket_fd) => c_result = send(socket_fd.clone(), self.buf.as_ptr() as _, self.buf.len(), 0) as i32
            }
        }

        if c_result != self.buf.len() as i32{
            return Err(CErr::new(ErrorKind::InvalidData, "WriteError"));
        }

        self.buf.clear();

        Ok(())
    }
}

impl Clone for Socket{
    fn clone(&self) -> Self {
        Socket{socket_file_descriptor: Rc::clone(&self.socket_file_descriptor), buf: self.buf.clone()}
    }
}
impl Drop for Socket {
    fn drop(&mut self) {
        unsafe {
            match self.socket_file_descriptor.get() {
                None => println!("Socket file descriptor already dropped."),
                Some(socket_fd) => {
                    let close_ret = close(socket_fd);
                    if close_ret == -1{
                        //The close documentation suggests it should not be retried after an error.
                        println!("Socket close on drop encountered error: {}, sock_fd: {}", CErr::last_os_error().raw_os_error().unwrap(), socket_fd);
                    }else{
                        println!("Socket closed successfully on drop.");
                    }
                }
            }

            self.socket_file_descriptor.replace(None);
        }
    }
}

pub mod u8_util{
    pub fn u8_to_str(u8_vec: &Vec<u8>) -> String {
        let mut my_char_vec: Vec<char> = Vec::with_capacity(u8_vec.len());
        for u in u8_vec{
            my_char_vec.push(u.clone() as char);
        }
        my_char_vec.iter().collect::<String>()
    }
}