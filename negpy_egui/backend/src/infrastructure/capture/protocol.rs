//! The Scanlight USB-CDC serial protocol codec.
//!
//! Mirrors `negpy/infrastructure/capture/protocol.py`, vendored from the Scanlight
//! firmware through TriRGB's `scanlightctl`. The whole Scanlight family shares this wire
//! format, so one codec drives every model.

/// The start byte that frames every packet.
pub const START_BYTE: u8 = 0xFE;

/// Host-to-device headers. NegPy never sends the firmware's `GET_DEFAULT_RGB`.
pub const H2D_SET_COLOR: u8 = 0;
pub const H2D_GET_FW_VERSION: u8 = 2;
pub const H2D_SHUTTER_PULSE: u8 = 3;

/// The pulse length byte counts 10 ms units, so the jack holds at most 2.55 s.
pub const SHUTTER_PULSE_UNIT_S: f64 = 0.01;
pub const MAX_SHUTTER_PULSE_S: f64 = 255.0 * SHUTTER_PULSE_UNIT_S;

/// Device-to-host headers.
pub const D2H_LED_TEMP: u8 = 1;
pub const D2H_FW_VERSION: u8 = 3;

/// A malformed packet or an out-of-range value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolError(pub String);

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ProtocolError {}

/// Frame one packet for transmission.
pub fn encode_packet(header: u8, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    if data.len() > 255 {
        return Err(ProtocolError(format!(
            "data too long: {} bytes",
            data.len()
        )));
    }
    let mut out = Vec::with_capacity(3 + data.len());
    out.extend_from_slice(&[START_BYTE, header, data.len() as u8]);
    out.extend_from_slice(data);
    Ok(out)
}

/// One `SET_COLOR` value per channel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub w: u8,
    pub ir: u8,
}

/// Build a `SET_COLOR` packet. `save` writes the values to NVM, which has finite write
/// cycles, so it must be opt-in.
pub fn encode_set_color(color: Color, save: bool) -> Result<Vec<u8>, ProtocolError> {
    encode_packet(
        H2D_SET_COLOR,
        &[color.r, color.g, color.b, color.w, color.ir, u8::from(save)],
    )
}

/// Build a shutter pulse that holds the line for `seconds`, rounded up to the firmware's
/// 10 ms unit.
pub fn encode_shutter_pulse(seconds: f64) -> Result<Vec<u8>, ProtocolError> {
    // Round to six places first, as the firmware encoder does, so a value a hair over
    // a unit boundary does not lose its last unit to floating-point error.
    let quotient = (seconds / SHUTTER_PULSE_UNIT_S * 1e6).round() / 1e6;
    let units = if (quotient - quotient.floor()).abs() < 1e-9 {
        quotient
    } else {
        quotient.floor() + 1.0
    };
    if !(1.0..=255.0).contains(&units) {
        return Err(ProtocolError(format!(
            "shutter pulse out of range 0.01-{MAX_SHUTTER_PULSE_S:.2} s: {seconds}"
        )));
    }
    encode_packet(H2D_SHUTTER_PULSE, &[units as u8])
}

/// An `LED_TEMP` payload in degrees Celsius: signed big-endian millidegrees.
pub fn decode_led_temp(data: &[u8]) -> Result<f64, ProtocolError> {
    let bytes: [u8; 4] = data
        .get(..4)
        .and_then(|slice| slice.try_into().ok())
        .ok_or_else(|| {
            ProtocolError(format!("LED_TEMP payload too short: {} bytes", data.len()))
        })?;
    Ok(f64::from(i32::from_be_bytes(bytes)) / 1000.0)
}

/// A `FW_VERSION` payload as `(firmware_id, hardware_id)`.
pub fn decode_fw_version(data: &[u8]) -> Result<(u16, u16), ProtocolError> {
    let bytes: [u8; 4] = data
        .get(..4)
        .and_then(|slice| slice.try_into().ok())
        .ok_or_else(|| {
            ProtocolError(format!(
                "FW_VERSION payload too short: {} bytes",
                data.len()
            ))
        })?;
    let word = u32::from_be_bytes(bytes);
    Ok(((word & 0xFFFF) as u16, ((word >> 16) & 0xFFFF) as u16))
}

const HARDWARE_NAMES: [&str; 4] = [
    "Big Scanlight",
    "Scanlight v4a",
    "Scanlight v2/v3",
    "Scanlight v4b",
];
const WHITE_CAPABLE_HW: [u16; 3] = [0, 1, 3];
const IR_CAPABLE_HW: [u16; 1] = [0];

/// The friendly name of a reported hardware ID, or `hw<n>` for an unknown one.
#[must_use]
pub fn describe_hardware(hw_id: u16) -> String {
    HARDWARE_NAMES
        .get(hw_id as usize)
        .map_or_else(|| format!("hw{hw_id}"), |name| (*name).to_string())
}

/// Whether this Scanlight model has infrared LEDs.
#[must_use]
pub fn has_ir_channel(hw_id: u16) -> bool {
    IR_CAPABLE_HW.contains(&hw_id)
}

/// Whether this Scanlight model has a dedicated white LED.
#[must_use]
pub fn has_white_channel(hw_id: u16) -> bool {
    WHITE_CAPABLE_HW.contains(&hw_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_packet_is_framed_with_its_start_byte_and_length() {
        assert_eq!(
            encode_packet(H2D_GET_FW_VERSION, &[]).unwrap(),
            vec![0xFE, 2, 0]
        );
    }

    #[test]
    fn a_set_color_carries_all_five_channels_and_the_save_flag() {
        let color = Color {
            r: 10,
            g: 20,
            b: 30,
            ..Color::default()
        };
        assert_eq!(
            encode_set_color(color, false).unwrap(),
            vec![0xFE, 0, 6, 10, 20, 30, 0, 0, 0]
        );
        assert_eq!(encode_set_color(color, true).unwrap()[8], 1);
    }

    #[test]
    fn the_decoders_read_the_firmware_words() {
        assert_eq!(decode_led_temp(&37_500i32.to_be_bytes()).unwrap(), 37.5);
        let word: u32 = (1 << 16) | 2;
        assert_eq!(decode_fw_version(&word.to_be_bytes()).unwrap(), (2, 1));
    }

    #[test]
    fn a_pulse_is_counted_in_ten_millisecond_units() {
        assert_eq!(encode_shutter_pulse(0.5).unwrap(), vec![0xFE, 3, 1, 50]);
        assert_eq!(encode_shutter_pulse(0.501).unwrap()[3], 51);
        assert!(encode_shutter_pulse(0.0).is_err());
        assert!(encode_shutter_pulse(2.6).is_err());
    }

    #[test]
    fn the_hardware_names_fall_back_to_the_raw_id() {
        assert_eq!(describe_hardware(0), "Big Scanlight");
        assert_eq!(describe_hardware(2), "Scanlight v2/v3");
        assert_eq!(describe_hardware(1), "Scanlight v4a");
        assert_eq!(describe_hardware(3), "Scanlight v4b");
        assert_eq!(describe_hardware(7), "hw7");
    }

    #[test]
    fn the_channels_follow_the_hardware_capability() {
        assert!(has_white_channel(0));
        assert!(has_white_channel(1));
        assert!(!has_white_channel(2));
        assert!(has_ir_channel(0));
        assert!(!has_ir_channel(1));
    }
}
