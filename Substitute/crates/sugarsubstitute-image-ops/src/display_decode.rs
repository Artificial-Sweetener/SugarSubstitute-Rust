// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode bounded display words without interpreting encoded RGB as linear image data.
use crate::{ImageError, Pixel, is_premultiplied_word};
use std::sync::LazyLock;

/// IEC sRGB inverse transfer in sRGB primaries; coverage is not passed through this function.
/// Source adapters validate their encoded samples before conversion. Negative encoded values
/// follow the linear segment, preserving the transfer's extended-range interpretation.
pub fn srgb_to_linear(encoded: f64) -> f32 {
    if encoded <= 0.04045 {
        (encoded / 12.92) as f32
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4) as f32
    }
}

/// Inverse transfer of every opaque 8-bit channel, computed once: display sources are almost
/// entirely opaque, and three transcendentals per pixel dominate decoding otherwise.
static OPAQUE: LazyLock<[f32; 256]> =
    LazyLock::new(|| std::array::from_fn(|channel| srgb_to_linear(channel as f64 / 255.0)));

/// Stateless conversion from retained premultiplied sRGB display pixels to image DAG pixels.
pub struct SrgbDecoder;
impl SrgbDecoder {
    /// Decode a premultiplied `0xAARRGGBB` word. RGB is unpremultiplied before inverse transfer,
    /// then premultiplied by linear coverage; transparent black remains exact.
    /// # Errors
    /// Rejects display words whose color exceeds their coverage.
    pub fn decode(word: u32) -> Result<Pixel, ImageError> {
        if !is_premultiplied_word(word) {
            return Err(ImageError::Invalid("Invalid premultiplied display source"));
        }
        let [alpha, red, green, blue] = word.to_be_bytes();
        if alpha == 0 {
            return Ok(Pixel::TRANSPARENT);
        }
        if alpha == 255 {
            let table = &*OPAQUE;
            return Pixel::linear(
                table[usize::from(red)],
                table[usize::from(green)],
                table[usize::from(blue)],
                1.0,
            );
        }
        let decode = |channel| srgb_to_linear(f64::from(channel) / f64::from(alpha));
        Pixel::linear(decode(red), decode(green), decode(blue), f32::from(alpha) / 255.0)
    }
    /// Decode a row of premultiplied `0xAARRGGBB` words into `output`, exactly as [`Self::decode`]
    /// decodes each. Sources read whole rows through this rather than one word at a time.
    /// # Errors
    /// Rejects rows of different lengths and any word whose color exceeds its coverage.
    pub fn decode_row(words: &[u32], output: &mut [Pixel]) -> Result<(), ImageError> {
        if words.len() != output.len() {
            return Err(ImageError::Invalid("Display row and its decoded row differ in length"));
        }
        let table = &*OPAQUE;
        for (pixel, word) in output.iter_mut().zip(words) {
            let [alpha, red, green, blue] = word.to_be_bytes();
            *pixel = if alpha == 255 {
                Pixel([
                    table[usize::from(red)],
                    table[usize::from(green)],
                    table[usize::from(blue)],
                    1.0,
                ])
            } else {
                Self::decode(*word)?
            };
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SrgbEncoder;

    #[test]
    fn display_decode_roundtrips_every_valid_grey_and_coverage() -> Result<(), ImageError> {
        for alpha in 1..=255_u8 {
            for channel in 0..=alpha {
                let pixel =
                    SrgbDecoder::decode(u32::from_be_bytes([alpha, channel, channel, channel]))?;
                let encoded = SrgbEncoder::encode_constant(pixel)?;
                assert_eq!(encoded[3], alpha);
                assert!(encoded[..3].iter().all(|value| value.abs_diff(channel) <= 1));
            }
        }
        assert_eq!(SrgbDecoder::decode(0)?.channels(), [0.0; 4]);
        assert!(SrgbDecoder::decode(0x007f0000).is_err());
        assert!(SrgbDecoder::decode(0x7f800000).is_err());
        Ok(())
    }

    #[test]
    fn inverse_transfer_preserves_linear_coverage_and_unpremultiplies_encoded_color()
    -> Result<(), ImageError> {
        let pixel = SrgbDecoder::decode(0x80404040)?.channels();
        let expected = 0.21404114 * (128.0 / 255.0);
        assert!((pixel[0] - expected).abs() < 1e-7);
        assert_eq!(pixel[3], 128.0 / 255.0);
        assert_eq!(srgb_to_linear(0.0), 0.0);
        assert_eq!(srgb_to_linear(1.0), 1.0);
        assert!((srgb_to_linear(0.04045) - 0.003130805).abs() < 1e-9);
        Ok(())
    }
}
