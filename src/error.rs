use std::fmt;
use std::fmt::Display;

#[derive(Debug, Copy, Clone, PartialEq)]
#[non_exhaustive]
pub enum Error
{
    InvalidParam,
    Access,
    Resource,
    NotFound,
    NoDevice,
    Busy,
}

impl Error
{
    /// Create an [Error] from a libwdi error code (e.g. [libwdi_sys::WDI_ERROR_NO_DEVICE].
    ///
    /// If there is not a corresponding error for the passed value, this function returns None.
    pub fn from_error_code(code: i32) -> Option<Self>
    {
        use libwdi_sys::*;
        use Error::*;

        match code {
            WDI_ERROR_INVALID_PARAM => Some(InvalidParam),
            WDI_ERROR_ACCESS => Some(Access),
            WDI_ERROR_RESOURCE => Some(Resource),
            WDI_ERROR_NO_DEVICE => Some(NoDevice),
            WDI_ERROR_NOT_FOUND => Some(NotFound),
            WDI_ERROR_BUSY => Some(Busy),
            _ => None,
        }
    }
}

impl Display for Error
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result
    {
        use Error::*;

        match self {
            InvalidParam => write!(f, "Invalid parameter")?,
            Access => write!(f, "Access denied (insufficient permissions)")?,
            NoDevice => write!(f, "No such device (it may have been disconnected)")?,
            NotFound => write!(f, "Entity not found")?,
            Busy => write!(f, "Resource busy, or API call already running")?,
            Resource => write!(f, "Could not acquire resource (insufficient memory, etc)")?,
        };

        Ok(())
    }
}

impl std::error::Error for Error { }
