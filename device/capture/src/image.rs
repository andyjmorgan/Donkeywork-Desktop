use crate::*;
use std::io::{self, Write};

pub(crate) fn rgba_len(width: u32, height: u32, limits: FrameLimits) -> Result<usize> {
    if width == 0
        || height == 0
        || width > limits.max_dimension.min(4096)
        || height > limits.max_dimension.min(4096)
    {
        return Err(BackendError::new(
            ErrorKind::ResourceExhausted,
            "frame dimensions exceed limits",
        ));
    }
    let size = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| BackendError::new(ErrorKind::ResourceExhausted, "frame size overflow"))?;
    if size > limits.max_rgba_bytes.min(64 * 1024 * 1024) {
        return Err(BackendError::new(
            ErrorKind::ResourceExhausted,
            "decoded frame exceeds limit",
        ));
    }
    Ok(size)
}

struct CappedWriter {
    data: Vec<u8>,
    limit: usize,
    exceeded: bool,
}
impl Write for CappedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.data.len()) {
            self.exceeded = true;
            return Err(io::Error::other("encoded PNG exceeds limit"));
        }
        self.data
            .try_reserve(bytes.len())
            .map_err(io::Error::other)?;
        self.data.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn encode_png(frame: &NativeFrame, limits: FrameLimits) -> Result<Vec<u8>> {
    let size = rgba_len(frame.width, frame.height, limits)?;
    if frame.rgba.len() != size {
        return Err(BackendError::new(
            ErrorKind::InvalidArgument,
            "RGBA length does not match frame dimensions",
        ));
    }
    let mut output = CappedWriter {
        data: Vec::new(),
        limit: limits.max_png_bytes.min(64 * 1024 * 1024),
        exceeded: false,
    };
    let result = (|| -> std::result::Result<(), png::EncodingError> {
        let mut encoder = png::Encoder::new(&mut output, frame.width, frame.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&frame.rgba)?;
        writer.finish()
    })();
    if output.exceeded {
        return Err(BackendError::new(
            ErrorKind::ResourceExhausted,
            "encoded PNG exceeds limit",
        ));
    }
    result.map_err(|e| BackendError::new(ErrorKind::CaptureFailed, e.to_string()))?;
    Ok(output.data)
}

#[derive(Clone, Copy)]
pub(crate) struct PixelFormat {
    pub bits_per_pixel: u8,
    pub scanline_pad: u8,
    pub little_endian: bool,
    pub red: u32,
    pub green: u32,
    pub blue: u32,
}

pub(crate) fn x11_to_rgba(
    bytes: &[u8],
    width: u32,
    height: u32,
    format: PixelFormat,
    limits: FrameLimits,
) -> Result<Vec<u8>> {
    let len = rgba_len(width, height, limits)?;
    if ![16, 24, 32].contains(&format.bits_per_pixel) || ![8, 16, 32].contains(&format.scanline_pad)
    {
        return Err(BackendError::new(
            ErrorKind::Unsupported,
            "unsupported X11 pixel packing",
        ));
    }
    let masks = [format.red, format.green, format.blue];
    for (i, &mask) in masks.iter().enumerate() {
        let normalized = mask >> mask.trailing_zeros().min(31);
        if mask == 0
            || normalized.count_ones() > 8
            || normalized & normalized.wrapping_add(1) != 0
            || (format.bits_per_pixel < 32 && mask >> format.bits_per_pixel != 0)
            || masks[..i].iter().any(|m| m & mask != 0)
        {
            return Err(BackendError::new(
                ErrorKind::Unsupported,
                "unsupported X11 colour masks",
            ));
        }
    }
    let pad = format.scanline_pad as usize;
    let stride = (width as usize * format.bits_per_pixel as usize).div_ceil(pad) * (pad / 8);
    let required = stride
        .checked_mul(height as usize)
        .ok_or_else(|| BackendError::new(ErrorKind::ResourceExhausted, "X11 stride overflow"))?;
    // X11 replies may end in at most three bytes of protocol padding.
    if bytes.len() < required || bytes.len() > required + 3 {
        return Err(BackendError::new(
            ErrorKind::CaptureFailed,
            "X11 image length does not match geometry",
        ));
    }
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(len)
        .map_err(|_| BackendError::new(ErrorKind::ResourceExhausted, "frame allocation failed"))?;
    let pixel_bytes = format.bits_per_pixel as usize / 8;
    for row in bytes[..required].chunks_exact(stride) {
        for pixel in row[..width as usize * pixel_bytes].chunks_exact(pixel_bytes) {
            let value = if format.little_endian {
                pixel
                    .iter()
                    .enumerate()
                    .fold(0u32, |value, (i, b)| value | (*b as u32) << (i * 8))
            } else {
                pixel.iter().fold(0u32, |value, b| (value << 8) | *b as u32)
            };
            for mask in masks {
                let shift = mask.trailing_zeros();
                let max = mask >> shift;
                rgba.push(((((value & mask) >> shift) * 255 + max / 2) / max) as u8);
            }
            rgba.push(255);
        }
    }
    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn format() -> PixelFormat {
        PixelFormat {
            bits_per_pixel: 32,
            scanline_pad: 32,
            little_endian: true,
            red: 0xff0000,
            green: 0xff00,
            blue: 0xff,
        }
    }
    fn frame(width: u32, height: u32, rgba: Vec<u8>) -> NativeFrame {
        NativeFrame {
            display_id: "1".into(),
            topology_revision: 1,
            captured_at: Instant::now(),
            width,
            height,
            cursor_embedded: false,
            rgba,
        }
    }
    #[test]
    fn little_and_big_endian_are_exact() {
        assert_eq!(
            x11_to_rgba(&[3, 2, 1, 0], 1, 1, format(), FrameLimits::default()).unwrap(),
            [1, 2, 3, 255]
        );
        let mut f = format();
        f.little_endian = false;
        assert_eq!(
            x11_to_rgba(&[0, 1, 2, 3], 1, 1, f, FrameLimits::default()).unwrap(),
            [1, 2, 3, 255]
        );
    }
    #[test]
    fn packed_rgb24_respects_scanline_padding() {
        let mut f = format();
        f.bits_per_pixel = 24;
        assert_eq!(
            x11_to_rgba(&[3, 2, 1, 99, 6, 5, 4, 88], 1, 2, f, FrameLimits::default()).unwrap(),
            [1, 2, 3, 255, 4, 5, 6, 255]
        );
    }
    #[test]
    fn rgb565_scales_channels() {
        let f = PixelFormat {
            bits_per_pixel: 16,
            scanline_pad: 16,
            little_endian: true,
            red: 0xf800,
            green: 0x7e0,
            blue: 0x1f,
        };
        assert_eq!(
            x11_to_rgba(
                &[0, 0xf8, 0xe0, 7, 0x1f, 0],
                3,
                1,
                f,
                FrameLimits::default()
            )
            .unwrap(),
            [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255]
        );
    }
    #[test]
    fn malformed_formats_and_lengths_fail() {
        let mut f = format();
        f.red = f.green;
        assert_eq!(
            x11_to_rgba(&[0; 4], 1, 1, f, FrameLimits::default())
                .unwrap_err()
                .kind,
            ErrorKind::Unsupported
        );
        f.red = 0x550000;
        assert_eq!(
            x11_to_rgba(&[0; 4], 1, 1, f, FrameLimits::default())
                .unwrap_err()
                .kind,
            ErrorKind::Unsupported
        );
        assert_eq!(
            x11_to_rgba(&[0; 3], 1, 1, format(), FrameLimits::default())
                .unwrap_err()
                .kind,
            ErrorKind::CaptureFailed
        );
        assert_eq!(
            x11_to_rgba(&[0; 8], 1, 1, format(), FrameLimits::default())
                .unwrap_err()
                .kind,
            ErrorKind::CaptureFailed
        );
    }
    #[test]
    fn decoded_bounds_are_independent_of_compression() {
        assert_eq!(
            rgba_len(3840, 2160, FrameLimits::default()).unwrap(),
            33_177_600
        );
        assert_eq!(
            rgba_len(4096, 4096, FrameLimits::default()).unwrap(),
            67_108_864
        );
        for (w, h) in [(0, 1), (1, 0), (4097, 1), (u32::MAX, u32::MAX)] {
            assert_eq!(
                rgba_len(w, h, FrameLimits::default()).unwrap_err().kind,
                ErrorKind::ResourceExhausted
            );
        }
        let limits = FrameLimits {
            max_rgba_bytes: 3,
            ..FrameLimits::default()
        };
        assert_eq!(
            rgba_len(1, 1, limits).unwrap_err().kind,
            ErrorKind::ResourceExhausted
        );
    }
    #[test]
    fn encoded_limit_and_wrong_rgba_length_fail() {
        let f = frame(1, 1, vec![255; 4]);
        assert!(encode_png(
            &f,
            FrameLimits {
                max_png_bytes: 16,
                ..FrameLimits::default()
            }
        )
        .is_err());
        assert_eq!(
            encode_png(&frame(1, 1, vec![0; 3]), FrameLimits::default())
                .unwrap_err()
                .kind,
            ErrorKind::InvalidArgument
        );
    }
    #[test]
    fn synthetic_native_4k_png_roundtrip_is_pixel_exact() {
        let rgba: Vec<u8> = (0..3840 * 2160)
            .flat_map(|n| {
                [
                    (n % 251) as u8,
                    ((n / 3840) % 251) as u8,
                    ((n / 17) % 251) as u8,
                    255,
                ]
            })
            .collect();
        let f = frame(3840, 2160, rgba);
        let encoded = encode_png(&f, FrameLimits::default()).unwrap();
        let mut reader = png::Decoder::new(std::io::Cursor::new(encoded))
            .read_info()
            .unwrap();
        let mut decoded = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut decoded).unwrap();
        assert_eq!(
            (info.width, info.height, info.color_type, info.bit_depth),
            (3840, 2160, png::ColorType::Rgba, png::BitDepth::Eight)
        );
        assert_eq!(decoded, f.rgba);
    }
}
