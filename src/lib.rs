//! Convert videos to MIDI for Roland Sound Canvas (SC-55/SC-8850/SD-90).

pub mod image;
pub mod process;
pub mod sysex;

use midly::Smf;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Target Roland Sound Canvas device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceMode {
    /// SC-55 / SC-88 (16x16 pixel canvas).
    Sc55,
    /// SC-8850 (160x64 pixel canvas).
    Sc8850,
    /// SD-90 (128x64 pixel canvas, reduced columns).
    Sd90,
}

/// Video conversion options.
pub struct Options {
    /// Output framerate (video passes through ffmpeg fps filter).
    pub framerate: u32,
    /// Roland device mode.
    pub mode: DeviceMode,
    /// Interlace: alternating section sets per frame (SC-8850 only).
    pub interlace: bool,
    /// Floyd-Steinberg dithering (SC-8850/SD-90 only).
    pub dither: bool,
    /// Sobel edge detection threshold, None = disabled (SC-8850/SD-90 only).
    pub edge: Option<u8>,
}

/// Single GRAY8 image -> complete Roland sysex messages.
///
/// No ffmpeg dependency. `data` must be tight-packed row-major pixels
/// at the resolution matching `mode` (256, 10240, or 8192 bytes).
pub fn convert_image(data: &[u8], mode: DeviceMode) -> Vec<Vec<u8>> {
    let binary = image::otsu_threshold(data);

    match mode {
        DeviceMode::Sc55 => {
            let raw = sysex::calculate_data(&binary);
            vec![sysex::pack_sysex_message(&raw)]
        }
        DeviceMode::Sc8850 => {
            let sections = sysex::calculate_data_8850(&binary);
            sysex::pack_sysex_message_8850(&sections)
        }
        DeviceMode::Sd90 => {
            let sections = sysex::calculate_data_sd90(&binary);
            sysex::pack_sysex_message_8850(&sections)
        }
    }
}

/// Video file -> MIDI Smf (caller saves with `smf.save(path)`).
pub fn convert_video(input: &str, options: &Options) -> Result<Smf<'static>> {
    process::video_to_smf(input, options)
}

/// Video file -> real-time MIDI port, paced by the fps filter tempo.
/// Requires `midi-output` feature (enabled by default).
#[cfg(feature = "midi-output")]
pub fn stream_video(input: &str, options: &Options, port_name: &str) -> Result<()> {
    process::video_to_port(input, options, port_name)
}
