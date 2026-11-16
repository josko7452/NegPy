//! Cross-platform monitor ICC detection.
//!
//! Mirrors `negpy/infrastructure/display/monitor_profile.py`: use the system profile
//! where the OS exposes it, and return `None` for sRGB on any failure.

/// The ICC bytes of the display's active profile, or `None` for sRGB.
///
/// `vendor` and `model` identify the screen on Linux, where colord holds several
/// displays. An empty pair takes the first display device.
#[must_use]
pub fn detect_monitor_icc(vendor: &str, model: &str) -> Option<Vec<u8>> {
    platform_profile(vendor, model)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn platform_profile(_vendor: &str, _model: &str) -> Option<Vec<u8>> {
    None
}

#[cfg(target_os = "linux")]
fn platform_profile(vendor: &str, model: &str) -> Option<Vec<u8>> {
    use zbus::blocking::{Connection, Proxy};
    use zbus::zvariant::{OwnedObjectPath, OwnedValue};

    const MANAGER: &str = "org.freedesktop.ColorManager";
    const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

    let connection = Connection::system().ok()?;
    let manager = Proxy::new(
        &connection,
        MANAGER,
        "/org/freedesktop/ColorManager",
        MANAGER,
    )
    .ok()?;
    let devices: Vec<OwnedObjectPath> = manager.call("GetDevicesByKind", &("display",)).ok()?;
    let devices: Vec<String> = devices.into_iter().map(|path| path.to_string()).collect();
    if devices.is_empty() {
        return None;
    }

    let get = |path: &str, interface: &str, name: &str| -> Option<OwnedValue> {
        let properties = Proxy::new(&connection, MANAGER, path, PROPERTIES).ok()?;
        properties.call("Get", &(interface, name)).ok()
    };
    let device_path =
        matching_device(&connection, &devices, vendor, model).unwrap_or_else(|| devices[0].clone());
    let profiles: Vec<OwnedObjectPath> =
        get(&device_path, &format!("{MANAGER}.Device"), "Profiles")?
            .try_into()
            .ok()?;
    let profile_path = profiles.first()?.to_string();
    let filename: String = get(&profile_path, &format!("{MANAGER}.Profile"), "Filename")?
        .try_into()
        .ok()?;
    return std::fs::read(filename).ok();

    fn matching_device(
        connection: &Connection,
        devices: &[String],
        vendor: &str,
        model: &str,
    ) -> Option<String> {
        use zbus::blocking::Proxy;
        use zbus::zvariant::OwnedValue;
        const MANAGER: &str = "org.freedesktop.ColorManager";
        const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
        let model = model.trim().to_lowercase();
        let vendor = vendor.trim().to_lowercase();
        if model.is_empty() && vendor.is_empty() {
            return None;
        }
        for path in devices {
            let read = |name: &str| -> Option<String> {
                let properties = Proxy::new(connection, MANAGER, path.as_str(), PROPERTIES).ok()?;
                let value: OwnedValue = properties
                    .call("Get", &(format!("{MANAGER}.Device"), name))
                    .ok()?;
                value.try_into().ok()
            };
            let device_model = read("Model").unwrap_or_default().to_lowercase();
            let device_vendor = read("Vendor").unwrap_or_default().to_lowercase();
            if !model.is_empty()
                && !device_model.is_empty()
                && (model.contains(&device_model) || device_model.contains(&model))
            {
                return Some(path.clone());
            }
            if !vendor.is_empty()
                && !device_vendor.is_empty()
                && vendor == device_vendor
                && model.is_empty()
            {
                return Some(path.clone());
            }
        }
        None
    }
}

#[cfg(target_os = "macos")]
fn platform_profile(_vendor: &str, _model: &str) -> Option<Vec<u8>> {
    use core_foundation::base::TCFType;
    use core_foundation::data::CFData;
    use core_graphics::display::CGDisplay;
    use std::os::raw::c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGDisplayCopyColorSpace(display: u32) -> *mut c_void;
        fn CGColorSpaceCopyICCData(space: *mut c_void) -> *mut c_void;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(value: *mut c_void);
    }

    let bytes = unsafe {
        let space = CGDisplayCopyColorSpace(CGDisplay::main_display().id);
        if space.is_null() {
            return None;
        }
        let data = CGColorSpaceCopyICCData(space);
        CFRelease(space);
        if data.is_null() {
            return None;
        }
        let data = CFData::wrap_under_create_rule(data.cast());
        let bytes = data.bytes().to_vec();
        (!bytes.is_empty()).then_some(bytes)
    };
    bytes
}

#[cfg(target_os = "windows")]
fn platform_profile(_vendor: &str, _model: &str) -> Option<Vec<u8>> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::Graphics::Gdi::{GetDC, ReleaseDC};
    use windows_sys::Win32::UI::ColorSystem::GetICMProfileW;

    let device = unsafe { GetDC(std::ptr::null_mut()) };
    if device.is_null() {
        return None;
    }
    let mut size = 0u32;
    let mut buffer: Vec<u16> = Vec::new();
    let ok = unsafe { GetICMProfileW(device, &mut size, std::ptr::null_mut()) };
    if ok == 0 {
        unsafe { ReleaseDC(std::ptr::null_mut(), device) };
        return None;
    }
    buffer.resize(size as usize, 0);
    let ok = unsafe { GetICMProfileW(device, &mut size, buffer.as_mut_ptr()) };
    unsafe { ReleaseDC(std::ptr::null_mut(), device) };
    if ok == 0 {
        return None;
    }
    let end = buffer
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(buffer.len());
    let path = std::path::PathBuf::from(std::ffi::OsString::from_wide(&buffer[..end]));
    std::fs::read(path).ok()
}

#[cfg(test)]
mod tests {
    use super::detect_monitor_icc;

    #[test]
    fn a_detected_profile_is_readable_icc() {
        if let Some(bytes) = detect_monitor_icc("", "") {
            assert!(qcms::Profile::new_from_slice(&bytes, false).is_some());
        }
    }
}
