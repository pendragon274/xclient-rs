#[warn(unused_imports)]
use std::fmt::{Display, Formatter};
use crate::{sock_read, sock_write};
use crate::sock::{SockError, Socket};

#[derive(Debug)]
pub enum XInterfaceError{
    SocketError(SockError),
    AuthFailure(String),
    AuthRequested(String),
    UnknownError
}

impl Display for XInterfaceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "XInterfaceError[{:?}]", self)
    }
}

impl From<SockError> for XInterfaceError{
    fn from(e: SockError) -> Self {
        XInterfaceError::SocketError(e)
    }
}

pub struct XInterface {
    x_socket: Socket
}

impl XInterface {
    /*fn get_x_auth_name(){
        let x_os_var = var_os("XAUTHORITY");
        match x_os_var {
            Some(x) => println!("OS Var: {:?}", x),
            None => println!("No OS var")
        }
    }*/

    fn auth_failure_read(&mut self) -> Result<String, XInterfaceError>{
        let reason_len = self.x_socket.read_serializable::<u8>(1)?;
        self.x_socket.read_serializable::<u16>(2)?; //Major version
        self.x_socket.read_serializable::<u16>(2)?; //Minor version
        let pad_size = self.x_socket.read_serializable::<u16>(2)?;
        let reason = self.x_socket.read_serializable::<String>(reason_len as usize)?;
        self.x_socket.read_discard_bytes((pad_size as usize * 4) - reason_len as usize)?;

        Ok(reason.trim().to_string())
    }

    fn auth_requested_read(&mut self) -> Result<String, XInterfaceError>{ //I honestly have zero idea what, if anything, would lead to this path, but it is in the documentation.
        self.x_socket.read_discard_bytes(5)?;
        let reason_len = self.x_socket.read_serializable::<u16>(2)?;
        let reason = self.x_socket.read_serializable::<String>(reason_len as usize)?;

        Ok(reason.trim().to_string())
    }

    #[allow(unused_variables)]
    fn auth_success_read(mut self) -> Result<Self, XInterfaceError>{
        //*****Preliminary Reads*****
        //self.x_socket.read_discard_bytes(1)?;
        sock_read!{self.x_socket; _:1, major_version[u16:2], minor_version[u16:2], pad_indicator[u16:2]};
        println!("Major: {}, minor: {}, pad_indicator: {}", major_version, minor_version, pad_indicator);
        //println!("Major Version from macro: {}", test_var);
        /*self.x_socket.read_discard_bytes(1)?;
        self.x_socket.read_serializable::<u16>(2)?; //Major version
        self.x_socket.read_serializable::<u16>(2)?; //Minor version
        let pad_indicator = self.x_socket.read_serializable::<u16>(2)?;              //8+2n+(v+p+m)/4
        let release_number = self.x_socket.read_serializable::<u32>(4)?;
        let resource_id_base = self.x_socket.read_serializable::<u32>(4)?;
        let resource_id_mask = self.x_socket.read_serializable::<u32>(4)?;
        let motion_buffer_size = self.x_socket.read_serializable::<u32>(4)?;
        let vendor_len = self.x_socket.read_serializable::<u16>(2)?;                 //v
        let maximum_request_length = self.x_socket.read_serializable::<u16>(2)?;
        let screen_count = self.x_socket.read_serializable::<u8>(1)?;
        let pixmap_format_count = self.x_socket.read_serializable::<u8>(1)?;         //n
        let image_byte_order = self.x_socket.read_serializable::<u8>(1)?;
        let bitmap_format_bit_order = self.x_socket.read_serializable::<u8>(1)?;
        let bitmap_format_scanline_unit = self.x_socket.read_serializable::<u8>(1)?;
        let bitmap_format_scanline_pad = self.x_socket.read_serializable::<u8>(1)?;
        let min_keycode = self.x_socket.read_serializable::<u8>(1)?;
        let max_keycode = self.x_socket.read_serializable::<u8>(1)?;
        self.x_socket.read_discard_bytes(4)?;
        let vendor = self.x_socket.read_serializable::<String>(vendor_len as usize)?;
        let pad_len = self.x_socket.read_pad(4)?;                  //p
        let mut pixmap_formats: Vec<Vec<u8>> = Vec::with_capacity(pixmap_format_count as usize);
        for _ in 0..pixmap_format_count{
            pixmap_formats.push(self.x_socket.read_bytes(8)?);
        }
        let _screens: Vec<Vec<u8>> = Vec::with_capacity(screen_count as usize);
        for _ in 0..screen_count {

        }*/
        //let roots = ((((pad_indicator-8)-(2*pixmap_format_count as u16))*4) - vendor_len) - pad_len as u16;

        Ok(self)
    }

    fn send_authentication(&mut self, auth_name: Vec<u8>, auth_data: Vec<u8>) -> Result<u8, XInterfaceError>{
        self.x_socket.write_all(vec![0x6C, 0, 11, 0, 0, 0])?;
        self.x_socket.write_serializable(auth_name.len() as u16)?;
        self.x_socket.write_serializable(auth_data.len() as u16)?;
        self.x_socket.write_all(vec![0, 0])?;
        self.x_socket.write_all(auth_name)?;
        self.x_socket.write_pad(4)?;
        self.x_socket.write_all(auth_data)?;
        self.x_socket.write_pad(4)?;
        self.x_socket.flush_all()?;

        Ok(self.x_socket.read_serializable::<u8>(1)?)
    }

    pub fn new(x_serv: &str) -> Result<Self, XInterfaceError> {
        println!("Initializing X interface.");
        let sock_connector = Socket::new(x_serv)?;
        let mut x_interface = XInterface{x_socket: sock_connector};

        let (auth_name, auth_data) = get_auth().unwrap();
        //println!("Auth Name: {:?}, auth data: {:?}", u8_util::u8_to_str(&auth_name), u8_util::u8_to_str(&auth_data));

        let auth_response = x_interface.send_authentication(auth_name, auth_data)?;

        match auth_response{
            0 => Err(XInterfaceError::AuthFailure(x_interface.auth_failure_read()?)),      //Failure
            1 => Ok(x_interface.auth_success_read()?),                                     //Success
            2 => Err(XInterfaceError::AuthRequested(x_interface.auth_requested_read()?)),  //Authentication Request
            _ => Err(XInterfaceError::UnknownError)                                        //Cursed
        }
    }
}

const MIT_MAGIC_COOKIE_1: &[u8] = b"MIT-MAGIC-COOKIE-1";

/// A family describes how to interpret some bytes as an address in an `AuthEntry`.
///
/// Compared to [`super::protocol::xproto::Family`], this is a `u16` and not an `u8` since
/// that's what is used in `~/.Xauthority` files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Family(u16);

#[warn(dead_code)]
impl Family {
    /// IPv4 connection to the server
    pub const INTERNET: Self = Self(0);
    /// DECnet
    pub const DEC_NET: Self = Self(1);
    /// Chaosnet connection
    pub const CHAOS: Self = Self(2);
    /// Family without predefined meaning, but interpreted by the server, for example a user name
    pub const SERVER_INTERPRETED: Self = Self(5);
    /// IPv6 connection to the server
    pub const INTERNET6: Self = Self(6);
    /// Wildcard matching any protocol family
    pub const WILD: Self = Self(65535);
    /// For local non-net authentication
    pub const LOCAL: Self = Self(256);
    /// TODO: No idea what this means exactly
    pub const NETNAME: Self = Self(254);
    /// Kerberos 5 principal name
    pub const KRB5_PRINCIPAL: Self = Self(253);
    /// For local non-net authentication
    pub const LOCAL_HOST: Self = Self(252);
}

#[warn(dead_code)]
impl From<u16> for Family {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

/// A single entry of an `.Xauthority` file.
#[derive(Debug, Clone, PartialEq, Eq)]
#[warn(dead_code)]
pub(crate) struct AuthEntry {
    /// The protocol family to which the entry applies
    family: Family,
    /// The address of the peer in a family-specific format
    address: Vec<u8>,
    /// The display number
    number: Vec<u8>,
    /// The name of the authentication method to use for the X11 server described by the previous
    /// fields.
    name: Vec<u8>,
    /// Extra data for the authentication method.
    data: Vec<u8>,
}

#[warn(dead_code)]
mod file {
    //! Code for actually reading `~/.Xauthority`.

    // use alloc::{vec, vec::Vec};
    use std::env::var_os;
    use std::fs::File;
    use std::io::{BufReader, Error, ErrorKind, Read};
    use std::path::PathBuf;

    use super::AuthEntry;

    /// Read a single `u16` from an `~/.Xauthority` file.
    ///
    /// The file stores these entries in big endian.
    fn read_u16<R: Read>(read: &mut R) -> Result<u16, Error> {
        let mut buffer = [0; 2];
        read.read_exact(&mut buffer)?;
        Ok(u16::from_be_bytes(buffer))
    }

    /// Read a single "byte array" from an `~/.Xauthority` file.
    ///
    /// The file stores these as a length field followed by a number of bytes that contain the
    /// actual data.
    fn read_string<R: Read>(read: &mut R) -> Result<Vec<u8>, Error> {
        let length = read_u16(read)?;
        let mut result = vec![0; length.into()];
        read.read_exact(&mut result[..])?;
        Ok(result)
    }

    /// Read a single entry from an `~/.Xauthority` file.
    ///
    /// This function tries to return `Ok(None)` when the end of the file is reached. However, the
    /// code also treats a single byte as 'end of file', because things were simpler to implement
    /// like this.
    fn read_entry<R: Read>(read: &mut R) -> Result<Option<AuthEntry>, Error> {
        let family = match read_u16(read) {
            Ok(family) => family,
            Err(ref e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        }
            .into();
        let address = read_string(read)?;
        let number = read_string(read)?;
        let name = read_string(read)?;
        let data = read_string(read)?;
        Ok(Some(AuthEntry {
            family,
            address,
            number,
            name,
            data,
        }))
    }

    /// Get the file name for `~/.Xauthority` based on environment variables.
    ///
    /// The code in libXau contains a special case for Windows (looks like cygwin) that is not
    /// handled here (yet?).
    fn get_xauthority_file_name() -> Option<PathBuf> {
        if let Some(name) = var_os("XAUTHORITY") {
            return Some(name.into());
        }
        var_os("HOME").map(|prefix| {
            let mut result = PathBuf::new();
            result.push(prefix);
            result.push(".Xauthority");
            result
        })
    }

    /// An iterator over the entries of an `.Xauthority` file
    #[derive(Debug)]
    pub(crate) struct XAuthorityEntries(BufReader<File>);

    impl XAuthorityEntries {
        /// Open `~/.Xauthority` for reading.
        ///
        /// This function returns `Ok(None)` when the location of the `.Xauthority` file could not
        /// be determined. If opening the file failed (for example, because it does not exist),
        /// that error is returned.
        pub(crate) fn new() -> Result<Option<XAuthorityEntries>, Error> {
            get_xauthority_file_name()
                .map(File::open)
                .transpose()?
                // At this point we have Option<File> and errors while opening the file were
                // returned to the caller.
                .map(|file| Ok(XAuthorityEntries(BufReader::new(file))))
                .transpose()
        }
    }

    impl Iterator for XAuthorityEntries {
        type Item = Result<AuthEntry, Error>;

        fn next(&mut self) -> Option<Self::Item> {
            read_entry(&mut self.0).transpose()
        }
    }
}

pub(crate) fn get_auth() -> Result<(Vec<u8>,Vec<u8>), u32>
{
    use file::XAuthorityEntries;
    let mut auth_proto_name = Vec::with_capacity(16);
    let mut auth_proto_data = Vec::with_capacity(16);
    let entries = XAuthorityEntries::new().unwrap().unwrap();
    for entry in entries {
        if entry.is_err() { continue; }
        let entry = entry.unwrap();
        if entry.name == MIT_MAGIC_COOKIE_1 {
            auth_proto_name.extend_from_slice(&entry.name);
            auth_proto_data.extend_from_slice(&entry.data);
            break;
        }
    }
    Ok((auth_proto_name,auth_proto_data))
}
