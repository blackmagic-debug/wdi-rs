//! High-ish level API to [libwdi](https://github.com/pbatard/libwdi).
//!
//! This crate is still extremely work in progress, but the major high level functions of interest
//! are [create_list] and [prepare_driver].

use std::ptr;
use std::ffi::{CString, CStr};
use std::fmt;
use std::fmt::Display;

use bstr::ByteSlice;

pub mod error;
pub use error::Error;

use libwdi_sys::wdi_device_info;


/// Provides information related to driver installation for a device.
///
/// Rust version of [libwdi_sys::wdi_device_info] ([original documentation]).
/// [original documentation]: https://github.com/pbatard/libwdi/wiki/Usage#struct_wdi_device_info
#[derive(Clone, PartialEq)]
pub struct DeviceInfo
{
    pub vid: u16,
    pub pid: u16,
    pub is_composite: bool,
    pub mi: u8,
    pub desc: Vec<u8>,
    pub driver: Option<Vec<u8>>,
    pub device_id: Option<Vec<u8>>,
    pub hardware_id: Option<Vec<u8>>,
    pub compatible_id: Option<Vec<u8>>,
    pub upper_filter: Option<Vec<u8>>,
    pub driver_version: u64,
}

/// Builder API.
//impl DeviceInfo
//{
    //pub fn driver(
//}

impl DeviceInfo
{
    // Performs a deep clone on the values in `raw` to construct a new DeviceInfo structure. You
    // probably want [create_list].
    //
    // Does *not* call [libwdi_sys::wdi_destroy_list] for you! Use [create_list] for that!
    ///
    /// # Panics:
    /// This function panics if `raw.desc` is null, as this field is mantatory in libwdi.
    pub fn clone_from_raw(raw: &wdi_device_info) -> Self
    {
        let desc = if !raw.desc.is_null() {
            unsafe { CStr::from_ptr(raw.desc) }
                .to_bytes_with_nul().to_vec()
        } else {
            panic!("Mantatory field wdi_device_info->desc is null");
        };
        let driver = if !raw.driver.is_null() {
            Some(unsafe { CStr::from_ptr(raw.driver) }
                .to_bytes_with_nul().to_vec()
            )
        } else {
            None
        };
        let device_id = if !raw.device_id.is_null() {
            Some(unsafe { CStr::from_ptr(raw.device_id) }
                .to_bytes_with_nul().to_vec()
            )
        } else {
            None
        };
        let hardware_id = if !raw.hardware_id.is_null() {
            Some(unsafe { CStr::from_ptr(raw.hardware_id) }
                .to_bytes_with_nul().to_vec()
            )
        } else {
            None
        };
        let compatible_id = if !raw.compatible_id.is_null() {
            Some(unsafe { CStr::from_ptr(raw.compatible_id) }
                .to_bytes_with_nul().to_vec()
            )
        } else {
            None
        };
        let upper_filter = if !raw.upper_filter.is_null() {
            Some(unsafe { CStr::from_ptr(raw.upper_filter) }
                .to_bytes_with_nul().to_vec()
            )
        } else {
            None
        };
        Self {
            vid: raw.vid,
            pid: raw.pid,
            is_composite: raw.is_composite != 0,
            mi: raw.mi,
            desc,
            driver,
            device_id,
            hardware_id,
            compatible_id,
            upper_filter,
            driver_version: raw.driver_version,
        }
    }

    /// Returns a [`wdi_device_info`] for this device info. `next` is set to NULL.
    ///
    /// The returned structure borrows from this one, and thus has the same lifetime as `self`.
    pub fn as_raw(&mut self) -> wdi_device_info
    {
        wdi_device_info {
            next: ptr::null_mut(),
            vid: self.vid,
            pid: self.pid,
            is_composite: self.is_composite as i32,
            mi: self.mi,
            desc: self.desc.as_mut_ptr() as *mut i8,
            driver: self.driver.as_mut().map(|v| v.as_mut_ptr()).unwrap_or(ptr::null_mut()) as *mut i8,
            device_id: self.device_id.as_mut().map(|v| v.as_mut_ptr()).unwrap_or(ptr::null_mut()) as *mut i8,
            hardware_id: self.hardware_id.as_mut().map(|v| v.as_mut_ptr()).unwrap_or(ptr::null_mut()) as *mut i8,
            compatible_id: self.compatible_id.as_mut().map(|v| v.as_mut_ptr()).unwrap_or(ptr::null_mut()) as *mut i8,
            upper_filter: self.upper_filter.as_mut().map(|v| v.as_mut_ptr()).unwrap_or(ptr::null_mut()) as *mut i8,
            driver_version: self.driver_version,
        }
    }
}

impl fmt::Debug for DeviceInfo
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result
    {
        f.debug_struct("DisplayInfo")
            .field("vid", &format!("0x{:04x}", self.vid))
            .field("pid", &format!("0x{:04x}", self.pid))
            .field("is_composite", &self.is_composite)
            .field("mi", &self.mi)
            .field("desc", &self.desc.as_bstr())
            .field("driver", &self.driver.as_ref().map(|s| s.as_bstr()))
            .field("device_id", &self.device_id.as_ref().map(|s| s.as_bstr()))
            .field("hardware_id", &self.hardware_id.as_ref().map(|s| s.as_bstr()))
            .field("compatible_id", &self.compatible_id.as_ref().map(|s| s.as_bstr()))
            .field("upper_filter", &self.upper_filter.as_ref().map(|s| s.as_bstr()))
            .field("driver_version", &self.driver_version)
            .finish()?;

        Ok(())
    }
}


/// Options for [create_list].
#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct CreateListOptions
{
    /// Enumerate all USB devices, instead of only devices without a driver installed.
    pub list_all: bool,

    /// Enumerate hubs in addition to normal USB devices.
    pub list_hubs: bool,

    /// Trim whitepsace for the device description string.
    pub trim_whitespaces: bool,
}

/// Functions for converting between this and [libwdi_sys::wdi_options_create_list].
impl CreateListOptions
{
    pub fn as_raw(self) -> libwdi_sys::wdi_options_create_list
    {
        libwdi_sys::wdi_options_create_list {
            list_all: self.list_all as i32,
            list_hubs: self.list_hubs as i32,
            trim_whitespaces: self.trim_whitespaces as i32,
        }
    }

    pub fn from_raw(raw: libwdi_sys::wdi_options_create_list) -> Self
    {
        Self {
            list_all: raw.list_all != 0,
            list_hubs: raw.list_hubs != 0,
            trim_whitespaces: raw.trim_whitespaces != 0,
        }
    }
}


/// A high-level Rust interface to [libwdi_sys::wdi_create_list] ([original documentation]).
///
/// Creates a [DeviceInfo] Vec of USB devices currently present on the system.
///
/// The `Vec<DeviceInfo>` this function returns is cloned from libwdi's address space,
/// rather than borrowing from it, in order to idiomatically manage the resources in Rust.
/// The overhead for this should be pretty trivial, but if you want to use the raw linked list,
/// feel free to call [libwdi_sys::wdi_create_list] yourself.
///
/// [original_documentation]:
/// https://github.com/pbatard/libwdi/wiki/Usage#int_wdi_create_liststruct_wdi_device_info_list_struct_wdi_options_create_list_options
pub fn create_list(options: CreateListOptions) -> Result<Vec<DeviceInfo>, Error>
{
    use libwdi_sys::{wdi_create_list, wdi_destroy_list};

    let mut list_ptr: *mut wdi_device_info = ptr::null_mut();

    let mut raw_opt = options.as_raw();

    if let Some(e) = Error::from_error_code(unsafe { wdi_create_list(&mut list_ptr, &mut raw_opt) }) {
        return Err(e);
    }

    // libwdi should never not set list_ptr for success cases, but we'd prefer to avoid
    // undefined behavior.
    if list_ptr.is_null() {
        panic!("wdi_create_list() return indicated success, but the list pointer is still null!");
    }


    let mut final_list: Vec<DeviceInfo> = Vec::new();

    let mut current = list_ptr;

    while !current.is_null() {

        let info = DeviceInfo::clone_from_raw(& unsafe { *current });
        final_list.push(info);

        current = unsafe { (*current).next };
    }

    if let Some(e) = Error::from_error_code(unsafe { wdi_destroy_list(list_ptr) } ) {
        return Err(e);
    }

    Ok(final_list)
}


#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(i32)]
pub enum DriverType
{
    /// WinUSB.sys.
    WinUsb  = libwdi_sys::WDI_WINUSB,

    /// libusb0.sys.
    Libusb0 = libwdi_sys::WDI_LIBUSB0,

    /// libusbK.sys.
    LibusbK = libwdi_sys::WDI_LIBUSBK,

    /// A custom user driver.
    User    = libwdi_sys::WDI_USER,
}

/// The error that occurs if [DriverType::from_raw] fails, including [DriverType]'s [TryFrom]
/// implementation.
#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct DriverTypeConversionError;

impl Display for DriverTypeConversionError
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result
    {
        write!(f, "attempted to convert an invalid WDI driver type value to DriverType")
    }
}

impl DriverType
{
    /// Constructs a [DriverType] from a raw libwdi driver type code, or None if `raw` is not a
    /// valid value of [DriverType].
    pub fn from_raw(raw: i32) -> Result<Self, DriverTypeConversionError>
    {
        use DriverType::*;

        match raw {
            libwdi_sys::WDI_WINUSB => Ok(WinUsb),
            libwdi_sys::WDI_LIBUSB0 => Ok(Libusb0),
            libwdi_sys::WDI_LIBUSBK => Ok(LibusbK),
            libwdi_sys::WDI_USER => Ok(User),
            _ => Err(DriverTypeConversionError)
        }
    }
}

impl TryFrom<i32> for DriverType
{
    type Error = DriverTypeConversionError;

    /// Create a [DriverType] from a WDI driver type error code.
    fn try_from(other: i32) -> Result<Self, Self::Error>
    {
        Self::from_raw(other)
    }
}

/// Defaults to [DriverType::WinUsb].
impl Default for DriverType
{
    fn default() -> Self
    {
        Self::WinUsb
    }
}


/// Options for [prepare_driver].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PrepareDriverOptions
{
    /// Type of driver to extract.
    driver_type: DriverType,

    /// A string to override the vendor name generated for the INF. Ultimately, this content appears
    /// under "Manufacturer" in the device manager's device properties.
    vendor_name: Option<Vec<u8>>,

    /// A GUID string (including braces) that is meant to override the DeviceGUID automatically
    /// generated by libwdi when creating a generic INF for WinUSB, libusb0 or libusbK.
    /// Be mindful that, if you need to force a DeviceGUID, this means that your device is no longer a generic one, in which case you should embed your driver file, including the static inf, as user files.
    device_guid: Option<Vec<u8>>,

    /// Disable generation of a cat file. Has no effect before Windows Vista.
    disable_cat: bool,

    /// Disable self-signing the cat file. Has no effect before Windows Vista.
    disable_signing: bool,

    /// The string to identify the autogenerated self-signed certificate.
    /// Default is `"CN=USB\VID_####&PID_####[&MI_##] (libwdi autogenerated)"`.
    cert_subject: Option<Vec<u8>>,

    /// Use a generic pre-installed WCID driver instead of a regular device-specific driver.
    use_wcid_driver: bool,

    /// Assume that `inf_name` passed to [prepare_driver] is a pre-existing INF to use, instead of
    /// generating one automatically.
    external_inf: bool,
}

/// Builder API.
impl PrepareDriverOptions
{
    /// Type of driver to extract.
    pub fn driver_type(self, driver_type: DriverType) -> Self
    {
        Self {
            driver_type,
            ..self
        }
    }

    /// A string to override the vendor name generated for the INF. Ultimately, this content appears
    /// under "Manufacturer" in the device manager's device properties.
    pub fn vendor_name(self, vendor_name: Option<CString>) -> Self
    {
        Self {
            vendor_name: vendor_name.map(|s| s.into_bytes_with_nul()),
            ..self
        }
    }

    /// A GUID string (including braces) that is meant to override the DeviceGUID automatically
    /// generated by libwdi when creating a generic INF for WinUSB, libusb0 or libusbK.
    /// Be mindful that, if you need to force a DeviceGUID, this means that your device is no longer a generic one, in which case you should embed your driver file, including the static inf, as user files.
    pub fn device_guid(self, device_guid: Option<CString>) -> Self
    {
        Self {
            device_guid: device_guid.map(|s| s.into_bytes_with_nul()),
            ..self
        }
    }

    /// Disable generation of a cat file. Has no effect before Windows Vista.
    pub fn disable_cat(self, disable_cat: bool) -> Self
    {
        Self {
            disable_cat,
            ..self
        }
    }

    /// Disable self-signing the cat file. Has no effect before Windows Vista.
    pub fn disable_signing(self, disable_signing: bool) -> Self
    {
        Self {
            disable_signing,
            ..self
        }
    }

    /// The string to identify the autogenerated self-signed certificate.
    /// Default is `"CN=USB\VID_####&PID_####[&MI_##] (libwdi autogenerated)"`.
    pub fn cert_subject(self, cert_subject: Option<CString>) -> Self
    {
        Self {
            cert_subject: cert_subject.map(|s| s.into_bytes_with_nul()),
            ..self
        }
    }

    /// Use a generic pre-installed WCID driver instead of a regular device-specific driver.
    pub fn use_wcid_driver(self, use_wcid_driver: bool) -> Self
    {
        Self {
            use_wcid_driver,
            ..self
        }
    }

    /// Assume that `inf_name` passed to [prepare_driver] is a pre-existing INF to use, instead of
    /// generating one automatically.
    pub fn external_inf(self, external_inf: bool) -> Self
    {
        Self {
            external_inf,
            ..self
        }
    }
}

/// Getters, with non-standard names due to the builder API.
impl PrepareDriverOptions
{
    pub fn get_driver_type(&self) -> DriverType
    {
        self.driver_type
    }

    pub fn get_vendor_name(&self) -> Option<&CStr>
    {
        self.vendor_name
            .as_ref()
            .map(|s| {
                CStr::from_bytes_with_nul(s.as_slice())
                    .expect("Unreachable: vendor_name cannot be an invalid C string")
            })
    }

    pub fn get_device_guid(&self) -> Option<&CStr>
    {
        self.device_guid
            .as_ref()
            .map(|s| {
                CStr::from_bytes_with_nul(s.as_slice())
                    .expect("Unreachable: device_guid cannot be an invalid C string")
            })
    }

    pub fn get_disable_cat(&self) -> bool
    {
        self.disable_cat
    }

    pub fn get_disable_signing(&self) -> bool
    {
        self.disable_signing
    }

    pub fn get_cert_subject(&self) -> Option<&CStr>
    {
        self.cert_subject
            .as_ref()
            .map(|s| {
                CStr::from_bytes_with_nul(s.as_slice())
                    .expect("Unreachable: cert_subject cannot be an invalid C string")
            })
    }

    pub fn get_use_wcid_driver(&self) -> bool
    {
        self.use_wcid_driver
    }

    pub fn get_external_inf(&self) -> bool
    {
        self.external_inf
    }
}

/// Functions for converting between this and [libwdi_sys::wdi_options_prepare_driver].
impl PrepareDriverOptions
{
    /// The returned [libwdi_sys::wdi_options_prepare_driver] borrows from this struct, and thus
    /// shares its lifetime.
    pub unsafe fn as_raw(&mut self) -> libwdi_sys::wdi_options_prepare_driver
    {

        let vendor_name = self.vendor_name.as_mut().map(|s| s.as_mut_ptr() as *mut i8).unwrap_or(ptr::null_mut());
        let device_guid = self.device_guid.as_mut().map(|s| s.as_mut_ptr() as *mut i8).unwrap_or(ptr::null_mut());
        let cert_subject = self.cert_subject.as_mut().map(|s| s.as_mut_ptr() as *mut i8).unwrap_or(ptr::null_mut());

        libwdi_sys::wdi_options_prepare_driver {
            driver_type: self.driver_type as i32,
            vendor_name,
            device_guid,
            disable_cat: self.disable_cat as i32,
            disable_signing: self.disable_signing as i32,
            cert_subject,
            use_wcid_driver: self.use_wcid_driver as i32,
            external_inf: self.external_inf as i32,
        }
    }

    /// Performs a deep clone on the values in `raw` to construct a new PrepareDriverOptions
    /// structure.
    ///
    /// # Panics:
    /// This function panics if `raw.driver_type` is not a valid value of [DriverType]. or if any
    ///
    /// # Undefined Behavior
    /// The strings in `raw` must be valid C strings, or null pointers.
    pub unsafe fn clone_from_raw(raw: &libwdi_sys::wdi_options_prepare_driver) -> Self
    {
        let vendor_name = if !raw.vendor_name.is_null() {
            Some(unsafe { CStr::from_ptr(raw.vendor_name) }
                    .to_bytes_with_nul().to_vec()
            )
        } else {
            None
        };

        let device_guid = if !raw.device_guid.is_null() {
            Some(unsafe { CStr::from_ptr(raw.device_guid) }
                .to_bytes_with_nul().to_vec()
            )
        } else {
            None
        };

        let cert_subject = if !raw.cert_subject.is_null() {
            Some(unsafe { CStr::from_ptr(raw.cert_subject) }
                .to_bytes_with_nul().to_vec()
            )
        } else {
            None
        };

        Self {
            driver_type: raw.driver_type.try_into().expect("invalid value for driver_type"),
            vendor_name,
            device_guid,
            disable_cat: raw.disable_cat != 0,
            disable_signing: raw.disable_signing != 0,
            cert_subject,
            use_wcid_driver: raw.use_wcid_driver != 0,
            external_inf: raw.external_inf != 0,
        }
    }
}


/// A Rust interface to [libwdi_sys::wdi_prepare_driver] ([original documentation]).
///
/// Extracts the driver files, and, where applicable, create the relevant INF for a specific device.
///
/// [original documentation]:
/// https://github.com/pbatard/libwdi/wiki/Usage#int_wdi_prepare_driverstruct_wdi_device_info_device_info_const_char_path_const_char_inf_name_struct_wdi_options_prepare_driver_options
pub fn prepare_driver(device: &mut DeviceInfo, path: &str, inf_name: &str, options: &mut PrepareDriverOptions) -> Result<(), Error>
{
    let mut raw = device.as_raw();

    // FIXME: these should probably just be the arguments.
    let cstr_path = CString::new(path).unwrap();
    let path_ptr = cstr_path.into_raw();
    let cstr_inf_name = CString::new(inf_name).unwrap();
    let inf_name_ptr = cstr_inf_name.into_raw();
    let mut opt = unsafe { options.as_raw() };

    let ret = unsafe { libwdi_sys::wdi_prepare_driver(&mut raw, path_ptr, inf_name_ptr, &mut opt) };

    drop(unsafe { CString::from_raw(path_ptr) });
    drop(unsafe { CString::from_raw(inf_name_ptr) });

    if let Some(e) = Error::from_error_code(ret) {
        return Err(e);
    }

    Ok(())
}


#[derive(Debug, Clone, PartialEq)]
pub struct InstallDriverOptions
{
    /// Handle to a Window application that should receive a modal progress dialog. When this
    /// parameter is provided, a modal progress dialog will be displayed for the duration of the
    /// driver installation process.
    hwnd: libwdi_sys::HWND,

    /// Install a filter driver instead of the regular driver (libusb-win32 only).
    install_filter_driver: bool,

    /// Number of milliseconds to wait for any pending installations. 0, means no timeout.
    pending_install_timeout: u32,
}

impl Default for InstallDriverOptions
{
    fn default() -> Self
    {
        Self {
            hwnd: ptr::null_mut(),
            install_filter_driver: false,
            pending_install_timeout: 0,
        }
    }
}

/// Functions for converting betwen this and [libwdi_sys::wdi_options_install_driver].
impl InstallDriverOptions
{
    pub fn as_raw(&mut self) -> libwdi_sys::wdi_options_install_driver
    {
        libwdi_sys::wdi_options_install_driver {
            hWnd: self.hwnd,
            install_filter_driver: self.install_filter_driver as i32,
            pending_install_timeout: self.pending_install_timeout,
        }
    }

    pub fn from_raw(other: &libwdi_sys::wdi_options_install_driver) -> Self
    {
        Self {
            hwnd: other.hWnd,
            install_filter_driver: other.install_filter_driver != 0,
            pending_install_timeout: other.pending_install_timeout,
        }
    }
}


/// A Rust interface to [libwdi_sys::wdi_install_driver] ([original_documentation]).
///
/// Performs the actual driver installation.
pub fn install_driver(device: &mut DeviceInfo, path: &str, inf_name: &str, options: &mut InstallDriverOptions) -> Result<(), Error>
{
    let mut raw = device.as_raw();

    // FIXME: these should probably just be the arguments.
    let cstr_path = CString::new(path).unwrap();
    let path_ptr = cstr_path.into_raw();
    let cstr_inf_name = CString::new(inf_name).unwrap();
    let inf_name_ptr = cstr_inf_name.into_raw();
    let mut opt = options.as_raw() ;

    let ret = unsafe { libwdi_sys::wdi_install_driver(&mut raw, path_ptr, inf_name_ptr, &mut opt) };

    drop(unsafe { CString::from_raw(path_ptr) });
    drop(unsafe { CString::from_raw(inf_name_ptr)});

    if let Some(e) = Error::from_error_code(ret) {
        return Err(e);
    }

    Ok(())
}
