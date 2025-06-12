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

impl From<XInterfaceError> for ApplicationError{
    fn from(x: XInterfaceError) -> Self {
        ApplicationError::XInterfaceError(x)
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
        Ok(Self {_x_interface: XInterface::new("/tmp/.X11-unix/X0")?})
    }
}