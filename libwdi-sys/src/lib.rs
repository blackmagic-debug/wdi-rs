use core::ptr;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct wdi_device_info
{
    pub next: *mut wdi_device_info,
    pub vid: u16,
    pub pid: u16,
    pub is_composite: u8, // BOOL
    pub mi: u8,
    pub desc: *mut u8,
    pub driver: *mut u8,
    pub device_id: *mut u8,
    pub hardware_id: *mut u8,
    pub compatible_id: *mut u8,
    pub upper_filter: *mut u8,
    pub driver_version: u64,
}

impl Default for wdi_device_info
{
    fn default() -> Self
    {
        Self {
            next: ptr::null_mut(),
            desc: ptr::null_mut(),
            driver: ptr::null_mut(),
            device_id: ptr::null_mut(),
            hardware_id: ptr::null_mut(),
            compatible_id: ptr::null_mut(),
            upper_filter: ptr::null_mut(),
            ..unsafe { std::mem::zeroed() }
            //..Default::default()
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct wdi_options_create_list
{
    pub list_all: u8, // BOOL
    pub list_hubs: u8, // BOOL
    pub trim_whitespace: u8, // BOOL
}



extern "C"
{
    pub fn wdi_create_list(list: *mut *mut wdi_device_info, options: *mut wdi_options_create_list) -> u32;
}
