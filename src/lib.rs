use std::ptr;
use std::ffi::CStr;

use libwdi_sys::wdi_device_info;


#[derive(Debug)]
pub struct DeviceInfo
{
    vid: u16,
    pid: u16,
    is_composite: bool,
    mi: u8,
    desc: String,
    driver: String,
    device_id: String,
    hardware_id: String,
    compatible_id: String,
    upper_filter: String,
    driver_version: u64,
}

impl DeviceInfo
{
    // Performs a deep clone on the values in `raw` to construct a new DeviceInfo structure. You
    // probably want [create_list].
    //
    // Does *not* call [libwdi_sys::wdi_destroy_list] for you! Use [create_list] for that!
    pub fn clone_from_raw(raw: &mut wdi_device_info) -> Self
    {
        let desc = unsafe { CStr::from_ptr(raw.desc) };
        let driver = unsafe { CStr::from_ptr(raw.driver) };
        let device_id = unsafe { CStr::from_ptr(raw.device_id) };
        let hardware_id = unsafe { CStr::from_ptr(raw.hardware_id) };
        let compatible_id = unsafe { CStr::from_ptr(raw.compatible_id) };
        let upper_filter = unsafe { CStr::from_ptr(raw.upper_filter) };
        Self {
            vid: raw.vid,
            pid: raw.pid,
            is_composite: raw.is_composite != 0,
            mi: raw.mi,
            desc: desc.to_string_lossy().to_string(),
            driver: driver.to_string_lossy().to_string(),
            device_id: device_id.to_string_lossy().to_string(),
            hardware_id: hardware_id.to_string_lossy().to_string(),
            compatible_id: compatible_id.to_string_lossy().to_string(),
            upper_filter: upper_filter.to_string_lossy().to_string(),
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
            driver: self.driver.as_mut_ptr() as *mut i8,
            device_id: self.device_id.as_mut_ptr() as *mut i8,
            hardware_id: self.hardware_id.as_mut_ptr() as *mut i8,
            compatible_id: self.compatible_id.as_mut_ptr() as *mut i8,
            upper_filter: self.upper_filter.as_mut_ptr() as *mut i8,
            driver_version: self.driver_version,
        }
    }
}


pub fn create_list(/* FIXME: options */) -> Vec<DeviceInfo>
{
    unimplemented!();
}
