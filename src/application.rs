//!Provides an interface for interacting with X11 and manages several key elements necessary for using it.
#[warn(unused_imports)]
use std::fmt::{Display, Formatter};
use crate::xinterface::{XInterface, XInterfaceError};

#[derive(Debug)]
pub enum ApplicationError{
    XInterfaceError(XInterfaceError)
}

impl Display for ApplicationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ApplicationError[{:?}]", self)
    }
}

///Handles initialization of connection to X11 and provides an interface for using it.
pub struct Application {
    _x_interface: XInterface
}

#[warn(missing_docs)]
impl Application {
    ///Generates a new application with an X11 connection.
    pub fn new() -> Result<Self, ApplicationError> {
        let unwrap_x: XInterface;
        let x_result = XInterface::new("/tmp/.X11-unix/X0");

        match x_result {
            Ok(x) => unwrap_x = x,
            Err(e) =>{
                println!("Connection to X server failed: {}", e);
                return Err(ApplicationError::XInterfaceError(e));
            }
        }

        //This line should be able to create an X window request.

        Ok(Self {_x_interface: unwrap_x})
    }
}