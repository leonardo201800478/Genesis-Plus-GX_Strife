// yx5200.rs
// Adaptação para o core RetroArch Genesis Plus GX

use crate::cd_stream::CdStream;
use crate::snd::{Blip, Sound};
use crate::shared::SharedState;
use minimp3::{Decoder, Frame};
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;

// YX5200 UART interface RX message size
const YX5200_RX_BUFFER_SIZE: usize = 10;

// YX5200 audio playback limitations
const YX5200_MAX_TRACK_INDEX: u16 = 2999;
const YX5200_MAX_VOLUME: u8 = 30;

// MP3 decoder context similar to minimp3_ex
struct Mp3Decoder {
    decoder: Option<Decoder<CdStream>>,
    info: Mp3Info,
    cur_sample: u64,
    last_error: i32,
}

struct Mp3Info {
    channels: u8,
    hz: u32,
    layer: u8,
    bitrate_kbps: u32,
}

impl Mp3Decoder {
    fn new() -> Self {
        Self {
            decoder: None,
            info: Mp3Info {
                channels: 0,
                hz: 0,
                layer: 0,
                bitrate_kbps: 0,
            },
            cur_sample: 0,
            last_error: 0,
        }
    }

    fn open_cb(&mut self, stream: CdStream) -> i32 {
        match Decoder::new(stream) {
            Ok(decoder) => {
                // Get info from first frame
                if let Ok(frame) = decoder.next_frame() {
                    self.info.channels = frame.channels as u8;
                    self.info.hz = frame.sample_rate as u32;
                    // For simplicity, we'll use constant values for other fields
                    self.info.layer = 3;
                    self.info.bitrate_kbps = 128;
                }
                self.decoder = Some(decoder);
                0
            }
            Err(_) => -1,
        }
    }

    fn read(&mut self, buffer: &mut [i16], samples: usize) -> usize {
        if let Some(decoder) = &mut self.decoder {
            let channels = self.info.channels as usize;
            let samples_needed = samples / channels;
            let mut samples_read = 0;
            
            while samples_read < samples_needed {
                match decoder.next_frame() {
                    Ok(Frame { data, sample_rate, channels: frame_channels, .. }) => {
                        // Update info if needed
                        if sample_rate as u32 != self.info.hz {
                            self.info.hz = sample_rate as u32;
                        }
                        
                        // Copy samples to buffer
                        let frame_samples = data.len();
                        let to_copy = std::cmp::min(frame_samples, samples - samples_read * channels);
                        
                        for i in 0..to_copy {
                            if samples_read * channels + i < buffer.len() {
                                buffer[samples_read * channels + i] = data[i];
                            }
                        }
                        
                        samples_read += to_copy / channels;
                        self.cur_sample += to_copy as u64 / channels as u64;
                        
                        if to_copy < frame_samples {
                            // Store remaining samples for next read
                            // This is simplified - actual implementation would need a buffer
                            break;
                        }
                    }
                    Err(e) => {
                        self.last_error = -1;
                        break;
                    }
                }
            }
            
            samples_read * channels
        } else {
            0
        }
    }

    fn seek(&mut self, sample: u64) -> bool {
        if let Some(decoder) = &mut self.decoder {
            // Note: minimp3 doesn't support direct seeking by sample
            // This is a simplified implementation
            self.cur_sample = sample;
            true
        } else {
            false
        }
    }

    fn close(&mut self) {
        self.decoder = None;
        self.info = Mp3Info {
            channels: 0,
            hz: 0,
            layer: 0,
            bitrate_kbps: 0,
        };
        self.cur_sample = 0;
        self.last_error = 0;
    }
}

pub struct Yx5200 {
    // RX data bits reception cycle (0-9)
    rx_cycle: u8,
    // RX message byte counter
    rx_counter: u8,
    // RX message buffer
    rx_buffer: [u8; YX5200_RX_BUFFER_SIZE],
    // audio playback status (false: stopped/paused, true: playing)
    playback_enabled: bool,
    // audio playback loop mode (false: single playback, true: repeated playback)
    playback_loop: bool,
    // audio playback volume (0-30)
    playback_volume: u8,
    // audio output (DAC) status (false: muted, true: enabled)
    audio_enabled: bool,
    // audio track index (0-2999, with 0 meaning no audio track loaded)
    track_index: u16,
    // left & right audio channels outputs
    audio: [i16; 2],
    // MP3 decoder
    mp3_decoder: Mp3Decoder,
    // Current MP3 file stream
    current_stream: Option<CdStream>,
    // ROM path for MP3 files
    rom_path: String,
}

impl Yx5200 {
    pub fn new(rom_path: &str) -> Self {
        Self {
            rx_cycle: 0,
            rx_counter: 0,
            rx_buffer: [0; YX5200_RX_BUFFER_SIZE],
            playback_enabled: false,
            playback_loop: false,
            playback_volume: 30,
            audio_enabled: true,
            track_index: 0,
            audio: [0; 2],
            mp3_decoder: Mp3Decoder::new(),
            current_stream: None,
            rom_path: rom_path.to_string(),
        }
    }

    pub fn init(&mut self, samplerate: u32, sound: &mut Sound) {
        // YX5200 audio playback rate depends on loaded audio track
        // Audio stream is resampled to desired rate using Blip Buffer
        if self.track_index != 0 {
            // Get blip buffer for channel 3 (assuming YX5200 uses channel 3)
            if let Some(blip) = sound.blips.get_mut(3) {
                blip.set_rates(self.mp3_decoder.info.hz as f64, samplerate as f64);
            }
        } else {
            // Set maximal MP3 samplerate when no audio track is loaded
            if let Some(blip) = sound.blips.get_mut(3) {
                blip.set_rates(48000.0, samplerate as f64);
            }
        }
    }

    pub fn reset(&mut self) {
        self.unload_track();
        *self = Yx5200::new(&self.rom_path);
        self.playback_volume = 30;
        self.audio_enabled = true;
    }

    pub fn write(&mut self, rx_data: u8) {
        // RX data byte transfer not started?
        if self.rx_cycle == 0 {
            // START bit received?
            if rx_data == 0 {
                // Initialize RX data byte reception
                self.rx_buffer[self.rx_counter as usize] = 0;
                self.rx_cycle = 1;
            }
        }
        // RX data byte transfer reception in progress?
        else if self.rx_cycle < 9 {
            // Update RX data byte with RX data line state (LSB first)
            self.rx_buffer[self.rx_counter as usize] |= rx_data << (self.rx_cycle - 1);
            self.rx_cycle += 1;
        }
        // RX data byte transfer finished?
        else {
            // STOP bit received?
            if rx_data != 0 {
                // Increment received byte counter
                self.rx_counter += 1;

                // RX message buffer filled?
                if self.rx_counter as usize == YX5200_RX_BUFFER_SIZE {
                    // Process RX message
                    self.process_cmd();

                    // Reinitialize RX message byte counter
                    self.rx_counter = 0;
                }
            }

            // Reset RX cycle
            self.rx_cycle = 0;
        }
    }

    pub fn update(&mut self, samples: u32, sound: &mut Sound) {
        // Previous audio outputs
        let prev_l = self.audio[0];
        let prev_r = self.audio[1];
        
        // Get number of needed YX5200 audio samples
        // This depends on the blip buffer implementation
        // For now, we'll use the input samples directly
        let samples_needed = samples as usize;

        // YX5200 audio playback started?
        if self.playback_enabled && self.track_index != 0 {
            let channels = self.mp3_decoder.info.channels as usize;
            let samples_to_read = samples_needed * channels;
            let mut audio_buffer = vec![0i16; samples_to_read];

            // Read needed audio samples
            let mut samples_available = self.mp3_decoder.read(&mut audio_buffer, samples_to_read);

            // Assume either end of audio file has been reached or decoding error occurred
            // if not all needed audio samples could be read
            if samples_available < samples_to_read {
                // Playback loop enabled and no MP3 decoding error?
                if self.playback_loop && self.mp3_decoder.last_error == 0 {
                    // Seek back to start of MP3 file
                    self.mp3_decoder.seek(0);
                    // Read more samples
                    let additional = self.mp3_decoder.read(
                        &mut audio_buffer[samples_available..],
                        samples_to_read - samples_available
                    );
                    samples_available += additional;
                } else {
                    // Stop audio playback
                    self.unload_track();
                    
                    // Add silent audio samples
                    for i in samples_available..samples_to_read {
                        audio_buffer[i] = 0;
                    }
                    samples_available = samples_to_read;
                }
            }

            // Check audio is not silent or muted
            if self.playback_volume > 0 && self.audio_enabled {
                // Update blip buffer with available audio samples
                let mut count = 0;
                for i in (0..samples_available).step_by(channels) {
                    let l = (audio_buffer[i] as i32 * self.playback_volume as i32) / YX5200_MAX_VOLUME as i32;
                    let r = if channels > 1 {
                        (audio_buffer[i + channels - 1] as i32 * self.playback_volume as i32) / YX5200_MAX_VOLUME as i32
                    } else {
                        l
                    };
                    
                    if let Some(blip) = sound.blips.get_mut(3) {
                        blip.add_delta_fast(count, (l - prev_l as i32) as i16, (r - prev_r as i32) as i16);
                    }
                    
                    count += 1;
                    self.audio[0] = l as i16;
                    self.audio[1] = r as i16;
                }
            } else {
                // Update blip buffer with silent audio output
                if let Some(blip) = sound.blips.get_mut(3) {
                    blip.add_delta_fast(0, -prev_l, -prev_r);
                }
                self.audio = [0, 0];
            }
        } else {
            // Update blip buffer with silent audio output
            if let Some(blip) = sound.blips.get_mut(3) {
                blip.add_delta_fast(0, -prev_l, -prev_r);
            }
            self.audio = [0, 0];
        }

        // End of blip buffer timeframe
        if let Some(blip) = sound.blips.get_mut(3) {
            blip.end_frame(samples_needed as i32);
        }
    }

    pub fn context_save(&self, state: &mut Vec<u8>) {
        // Save YX5200 state
        state.extend_from_slice(&self.rx_cycle.to_le_bytes());
        state.extend_from_slice(&self.rx_counter.to_le_bytes());
        state.extend_from_slice(&self.rx_buffer);
        state.extend_from_slice(&[self.playback_enabled as u8]);
        state.extend_from_slice(&[self.playback_loop as u8]);
        state.extend_from_slice(&[self.playback_volume]);
        state.extend_from_slice(&[self.audio_enabled as u8]);
        state.extend_from_slice(&self.track_index.to_le_bytes());
        state.extend_from_slice(&self.audio[0].to_le_bytes());
        state.extend_from_slice(&self.audio[1].to_le_bytes());
        
        // Save MP3 decoder state
        state.extend_from_slice(&self.mp3_decoder.cur_sample.to_le_bytes());
    }

    pub fn context_load(&mut self, state: &[u8]) -> Result<usize, &'static str> {
        let mut offset = 0;
        
        // Load YX5200 state
        if state.len() < offset + 1 { return Err("Invalid state data"); }
        self.rx_cycle = state[offset]; offset += 1;
        
        if state.len() < offset + 1 { return Err("Invalid state data"); }
        self.rx_counter = state[offset]; offset += 1;
        
        if state.len() < offset + YX5200_RX_BUFFER_SIZE { return Err("Invalid state data"); }
        self.rx_buffer.copy_from_slice(&state[offset..offset + YX5200_RX_BUFFER_SIZE]);
        offset += YX5200_RX_BUFFER_SIZE;
        
        if state.len() < offset + 1 { return Err("Invalid state data"); }
        self.playback_enabled = state[offset] != 0; offset += 1;
        
        if state.len() < offset + 1 { return Err("Invalid state data"); }
        self.playback_loop = state[offset] != 0; offset += 1;
        
        if state.len() < offset + 1 { return Err("Invalid state data"); }
        self.playback_volume = state[offset]; offset += 1;
        
        if state.len() < offset + 1 { return Err("Invalid state data"); }
        self.audio_enabled = state[offset] != 0; offset += 1;
        
        if state.len() < offset + 2 { return Err("Invalid state data"); }
        self.track_index = u16::from_le_bytes([state[offset], state[offset + 1]]);
        offset += 2;
        
        if state.len() < offset + 2 { return Err("Invalid state data"); }
        self.audio[0] = i16::from_le_bytes([state[offset], state[offset + 1]]);
        offset += 2;
        
        if state.len() < offset + 2 { return Err("Invalid state data"); }
        self.audio[1] = i16::from_le_bytes([state[offset], state[offset + 1]]);
        offset += 2;
        
        // Load MP3 decoder state
        if state.len() < offset + 8 { return Err("Invalid state data"); }
        let cur_sample = u64::from_le_bytes([
            state[offset], state[offset + 1], state[offset + 2], state[offset + 3],
            state[offset + 4], state[offset + 5], state[offset + 6], state[offset + 7]
        ]);
        offset += 8;
        
        // Reload track if needed
        let index = self.track_index;
        self.track_index = 0;
        if index > 0 && index <= YX5200_MAX_TRACK_INDEX {
            self.load_track(index, self.playback_loop);
            self.mp3_decoder.seek(cur_sample);
        }
        
        Ok(offset)
    }

    fn process_cmd(&mut self) {
        // Process command code (assume message format and checksum are always correct)
        match self.rx_buffer[3] {
            0x01 => {
                // Play next track (only if there is already a track loaded)
                if self.track_index > 0 && self.track_index <= YX5200_MAX_TRACK_INDEX {
                    self.load_track(self.track_index + 1, self.playback_loop);
                }
            }
            0x02 => {
                // Play previous track
                if self.track_index > 1 {
                    self.load_track(self.track_index - 1, self.playback_loop);
                }
            }
            0x03 => {
                // Play selected track (1-2999)
                let index = (self.rx_buffer[5] as u16) << 8 | self.rx_buffer[6] as u16;
                if index > 0 && index <= YX5200_MAX_TRACK_INDEX {
                    // Playback loop seems to be enabled by default
                    self.load_track(index, true);
                }
            }
            0x04 => {
                // Increase playback volume
                if self.playback_volume < YX5200_MAX_VOLUME {
                    self.playback_volume += 1;
                }
            }
            0x05 => {
                // Decrease playback volume
                if self.playback_volume > 0 {
                    self.playback_volume -= 1;
                }
            }
            0x06 => {
                // Set playback volume (0-30)
                let volume = (self.rx_buffer[5] as u16) << 8 | self.rx_buffer[6] as u16;
                if volume <= YX5200_MAX_VOLUME as u16 {
                    self.playback_volume = volume as u8;
                }
            }
            0x08 => {
                // Single-repeat selected track (1-2999)
                let index = (self.rx_buffer[5] as u16) << 8 | self.rx_buffer[6] as u16;
                if index > 0 && index <= YX5200_MAX_TRACK_INDEX {
                    // Purpose of this command is not clear in available documentation so,
                    // to differentiate it from command 0x03 which appears to play selected
                    // audio track in loop, assume 'single repeat' playback means selected
                    // audio track is played only once (to be confirmed)
                    self.load_track(index, false);
                }
            }
            0x0c => {
                // Reset
                self.reset();
            }
            0x0d => {
                // Resume audio playback
                if self.track_index != 0 {
                    self.playback_enabled = true;
                }
            }
            0x0e => {
                // Pause audio playback
                self.playback_enabled = false;
            }
            0x16 => {
                // Stop audio playback
                self.unload_track();
            }
            0x19 => {
                // Enable/disable current audio track loop playback (during playback only)
                if self.playback_enabled {
                    self.playback_loop = (self.rx_buffer[6] & 0x01) == 0;
                }
            }
            0x1a => {
                // Enable/disable DAC output
                self.audio_enabled = (self.rx_buffer[6] & 0x01) == 0;
            }
            _ => {
                // Unsupported command
            }
        }
    }

    fn load_track(&mut self, index: u16, playback_loop: bool) {
        // First stop any audio playback
        self.unload_track();

        // Supported filename formats (max 2999 tracks)
        let formats = ["{:01}.mp3", "{:02}.mp3", "{:03}.mp3", "{:04}.mp3"];

        // Attempt to open MP3 file
        for fmt in formats.iter() {
            let filename = fmt.replace("{}", &index.to_string());
            let full_path = format!("{}/{}", self.rom_path, filename);
            
            if let Ok(stream) = CdStream::open(&full_path) {
                // Attempt to initialize MP3 file decoder
                if self.mp3_decoder.open_cb(stream) == 0 {
                    // Valid MP3 file?
                    if self.mp3_decoder.info.channels > 0 && self.mp3_decoder.info.channels <= 2 {
                        // Indicate audio track is loaded
                        self.track_index = index;
                        
                        // Start audio playback
                        self.playback_enabled = true;
                        
                        // Set playback loop mode
                        self.playback_loop = playback_loop;
                        
                        // Store current stream
                        self.current_stream = Some(CdStream::open(&full_path).unwrap());
                    } else {
                        // Close stream if not a valid MP3
                        // Stream will be closed when dropped
                    }
                }
                
                // Exit loop when MP3 file has been found
                break;
            }
        }
    }

    fn unload_track(&mut self) {
        // Check audio track is loaded
        if self.track_index != 0 {
            // Close MP3 file decoder
            self.mp3_decoder.close();
            
            // Close MP3 file stream
            self.current_stream = None;
            
            // Stop audio playback
            self.playback_enabled = false;
            
            // No audio track loaded
            self.track_index = 0;
        }
    }
}

// Interface functions for C compatibility (FFI)
pub mod ffi {
    use super::*;
    use std::ffi::CStr;
    use std::os::raw::c_char;
    
    #[no_mangle]
    pub extern "C" fn yx5200_init(yx5200: &mut Yx5200, samplerate: u32, sound: &mut Sound) {
        yx5200.init(samplerate, sound);
    }
    
    #[no_mangle]
    pub extern "C" fn yx5200_reset(yx5200: &mut Yx5200) {
        yx5200.reset();
    }
    
    #[no_mangle]
    pub extern "C" fn yx5200_write(yx5200: &mut Yx5200, rx_data: u8) {
        yx5200.write(rx_data);
    }
    
    #[no_mangle]
    pub extern "C" fn yx5200_update(yx5200: &mut Yx5200, samples: u32, sound: &mut Sound) {
        yx5200.update(samples, sound);
    }
    
    #[no_mangle]
    pub extern "C" fn yx5200_create(rom_path: *const c_char) -> *mut Yx5200 {
        let rom_path_cstr = unsafe { CStr::from_ptr(rom_path) };
        let rom_path_str = rom_path_cstr.to_str().unwrap_or("");
        Box::into_raw(Box::new(Yx5200::new(rom_path_str)))
    }
    
    #[no_mangle]
    pub extern "C" fn yx5200_destroy(yx5200: *mut Yx5200) {
        if !yx5200.is_null() {
            unsafe { Box::from_raw(yx5200); }
        }
    }
}