use serde::{Deserialize, Serialize};
pub mod input;
pub mod input_device;
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt},
    path::Path,
};

pub const PROTOCOL: &str = "dwconsole.stream";
pub const VERSION: &str = "0.1.0";
pub const MAX_HEADER: usize = 64 * 1024;
pub const MAX_CHUNK: usize = 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StreamHeader {
    pub protocol: String,
    pub version: String,
    pub codec: String,
    pub bitstream: String,
    pub encoder: String,
    pub display_id: String,
    pub device: String,
    pub width: u32,
    pub height: u32,
    pub frames_per_second: u32,
    pub pixel_format: String,
}

impl StreamHeader {
    pub fn validate(&self) -> io::Result<()> {
        if self.protocol != PROTOCOL
            || self.version != VERSION
            || self.codec != "h264"
            || self.bitstream != "annex-b"
            || self.width == 0
            || self.height == 0
            || self.width > 4096
            || self.height > 4096
            || !(1..=60).contains(&self.frames_per_second)
            || self.display_id.is_empty()
            || self.device.is_empty()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid stream header",
            ));
        }
        Ok(())
    }
}

pub fn write_record(mut writer: impl Write, bytes: &[u8], maximum: usize) -> io::Result<()> {
    if bytes.is_empty() || bytes.len() > maximum || bytes.len() > u32::MAX as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "record size rejected",
        ));
    }
    writer.write_all(&(bytes.len() as u32).to_be_bytes())?;
    writer.write_all(bytes)
}

pub fn read_record(mut reader: impl Read, maximum: usize) -> io::Result<Vec<u8>> {
    let mut length = [0_u8; 4];
    reader.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > maximum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "record size rejected",
        ));
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}

pub fn write_header(writer: impl Write, header: &StreamHeader) -> io::Result<()> {
    header.validate()?;
    let bytes = serde_json::to_vec(header)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "cannot serialize header"))?;
    write_record(writer, &bytes, MAX_HEADER)
}

pub fn read_header(reader: impl Read) -> io::Result<StreamHeader> {
    let bytes = read_record(reader, MAX_HEADER)?;
    let mut deserializer = serde_json::Deserializer::from_slice(&bytes);
    let header = StreamHeader::deserialize(&mut deserializer)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid stream header"))?;
    deserializer
        .end()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "trailing stream header data"))?;
    header.validate()?;
    Ok(header)
}

pub fn private_create(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
}

pub fn validate_root_socket(path: &Path) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "socket path must be absolute",
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "socket requires parent"))?;
    let parent_meta = std::fs::symlink_metadata(parent)?;
    let socket_meta = std::fs::symlink_metadata(path)?;
    if !parent_meta.is_dir()
        || parent_meta.uid() != 0
        || parent_meta.mode() & 0o022 != 0
        || !socket_meta.file_type().is_socket()
        || socket_meta.uid() != 0
        || socket_meta.mode() & 0o077 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe console socket",
        ));
    }
    Ok(())
}

pub fn repack_bgr0(data: &[u8], width: u32, height: u32, stride: u32) -> io::Result<Vec<u8>> {
    let row = (width as usize)
        .checked_mul(4)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "frame overflow"))?;
    let stride = stride as usize;
    let height = height as usize;
    let source = stride
        .checked_mul(height)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "frame overflow"))?;
    if width == 0
        || height == 0
        || width > 4096
        || height > 4096
        || stride < row
        || data.len() < source
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid frame layout",
        ));
    }
    if stride == row {
        return Ok(data[..source].to_vec());
    }
    let mut packed = Vec::with_capacity(row * height);
    for y in 0..height {
        packed.extend_from_slice(&data[y * stride..y * stride + row]);
    }
    Ok(packed)
}

pub fn contains_annex_b_start_code(data: &[u8]) -> bool {
    data.windows(4).any(|w| w == [0, 0, 0, 1]) || data.windows(3).any(|w| w == [0, 0, 1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_is_bounded_and_round_trips() {
        let mut bytes = Vec::new();
        write_record(&mut bytes, b"abc", 3).unwrap();
        assert_eq!(read_record(bytes.as_slice(), 3).unwrap(), b"abc");
        assert!(write_record(Vec::new(), b"abcd", 3).is_err());
        assert!(read_record([0, 0, 0, 4, 1, 2, 3, 4].as_slice(), 3).is_err());
    }

    #[test]
    fn padded_frames_are_repacked_without_changing_pixels() {
        let data = [
            1, 2, 3, 4, 5, 6, 7, 8, 99, 99, 9, 10, 11, 12, 13, 14, 15, 16, 99, 99,
        ];
        assert_eq!(
            repack_bgr0(&data, 2, 2, 10).unwrap(),
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
        );
        assert!(repack_bgr0(&data, 3, 2, 10).is_err());
    }

    #[test]
    fn annex_b_detection_requires_a_start_code() {
        assert!(contains_annex_b_start_code(&[7, 0, 0, 0, 1, 0x67]));
        assert!(contains_annex_b_start_code(&[0, 0, 1, 0x65]));
        assert!(!contains_annex_b_start_code(&[1, 2, 3, 4]));
    }
}
