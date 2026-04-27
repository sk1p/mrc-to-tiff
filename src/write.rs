use std::{
    error::Error,
    path::{Path, PathBuf},
};

use byteorder::{BigEndian, LittleEndian, WriteBytesExt};
use tiff_encoder::{
    LONG, RATIONAL, SHORT, TiffFile,
    ifd::{Ifd, tags},
    write::ByteBlock,
};

use crate::common::OutputEndianess;

#[derive(Debug, thiserror::Error)]
enum WriteError {
    #[error("file {path:?} already exists")]
    FileAlreadyExists { path: PathBuf },
}

pub trait OutputDtype {
    const BITS_PER_SAMPLE: u16;
    const BYTES_PER_SAMPLE: usize;
    const SAMPLE_FORMAT: SampleFormat;

    fn write_le(&self, dest: &mut impl WriteBytesExt);
    fn write_be(&self, dest: &mut impl WriteBytesExt);
}

impl OutputDtype for u8 {
    const BITS_PER_SAMPLE: u16 = 8;

    const BYTES_PER_SAMPLE: usize = 1;

    const SAMPLE_FORMAT: SampleFormat = SampleFormat::UInt;

    fn write_le(&self, dest: &mut impl WriteBytesExt) {
        dest.write_u8(*self);
    }

    fn write_be(&self, dest: &mut impl WriteBytesExt) {
        dest.write_u8(*self);
    }
}

impl OutputDtype for i8 {
    const BITS_PER_SAMPLE: u16 = 8;

    const BYTES_PER_SAMPLE: usize = 1;

    const SAMPLE_FORMAT: SampleFormat = SampleFormat::Int;

    fn write_le(&self, dest: &mut impl WriteBytesExt) {
        dest.write_i8(*self);
    }

    fn write_be(&self, dest: &mut impl WriteBytesExt) {
        dest.write_i8(*self);
    }
}

impl OutputDtype for u16 {
    const BITS_PER_SAMPLE: u16 = 16;

    const BYTES_PER_SAMPLE: usize = 2;

    const SAMPLE_FORMAT: SampleFormat = SampleFormat::UInt;

    fn write_le(&self, dest: &mut impl WriteBytesExt) {
        dest.write_u16::<LittleEndian>(*self);
    }

    fn write_be(&self, dest: &mut impl WriteBytesExt) {
        dest.write_u16::<BigEndian>(*self);
    }
}

impl OutputDtype for i16 {
    const BITS_PER_SAMPLE: u16 = 16;

    const BYTES_PER_SAMPLE: usize = 2;

    const SAMPLE_FORMAT: SampleFormat = SampleFormat::Int;

    fn write_le(&self, dest: &mut impl WriteBytesExt) {
        dest.write_i16::<LittleEndian>(*self);
    }

    fn write_be(&self, dest: &mut impl WriteBytesExt) {
        dest.write_i16::<BigEndian>(*self);
    }
}

impl OutputDtype for u32 {
    const BITS_PER_SAMPLE: u16 = 32;

    const BYTES_PER_SAMPLE: usize = 4;

    const SAMPLE_FORMAT: SampleFormat = SampleFormat::UInt;

    fn write_le(&self, dest: &mut impl WriteBytesExt) {
        dest.write_u32::<LittleEndian>(*self);
    }

    fn write_be(&self, dest: &mut impl WriteBytesExt) {
        dest.write_u32::<BigEndian>(*self);
    }
}

impl OutputDtype for i32 {
    const BITS_PER_SAMPLE: u16 = 32;

    const BYTES_PER_SAMPLE: usize = 4;

    const SAMPLE_FORMAT: SampleFormat = SampleFormat::Int;

    fn write_le(&self, dest: &mut impl WriteBytesExt) {
        dest.write_i32::<LittleEndian>(*self);
    }

    fn write_be(&self, dest: &mut impl WriteBytesExt) {
        dest.write_i32::<BigEndian>(*self);
    }
}


impl OutputDtype for f32 {
    const BITS_PER_SAMPLE: u16 = 32;

    const BYTES_PER_SAMPLE: usize = 4;

    const SAMPLE_FORMAT: SampleFormat = SampleFormat::Float;

    fn write_le(&self, dest: &mut impl WriteBytesExt) {
        dest.write_f32::<LittleEndian>(*self);
    }

    fn write_be(&self, dest: &mut impl WriteBytesExt) {
        dest.write_f32::<BigEndian>(*self);
    }
}

#[repr(u16)]
pub enum SampleFormat {
    UInt = 1,
    Int = 2,
    Float = 3,
    Undefined = 4,
}

/// Write a slice of data to a tiff file. The data must already be in the destination type,
/// but a destination endianess can be specified.
pub fn write_tiff<T>(
    filename: &Path,
    data: &[T],
    width: usize,
    height: usize,
    endianness: OutputEndianess,
) -> Result<(), Box<dyn Error + Sync + Send>>
where
    T: OutputDtype,
{
    if filename.exists() {
        return Err(Box::new(WriteError::FileAlreadyExists {
            path: filename.to_owned(),
        }));
    }

    let mut image_bytes: Vec<u8> = Vec::with_capacity(width * height * T::BYTES_PER_SAMPLE);
    match endianness.for_encoding() {
        tiff_encoder::write::Endianness::MM => {
            for value in data.iter() {
                value.write_be(&mut image_bytes);
            }
        }
        tiff_encoder::write::Endianness::II => {
            for value in data.iter() {
                value.write_le(&mut image_bytes);
            }
        }
    }

    TiffFile::new(
        Ifd::new()
            .with_entry(tags::PhotometricInterpretation, SHORT![1]) // Black is zero
            .with_entry(tags::Compression, SHORT![1]) // No compression
            .with_entry(tags::BitsPerSample, SHORT![T::BITS_PER_SAMPLE])
            .with_entry(tags::SamplesPerPixel, SHORT![1])
            .with_entry(tags::SampleFormat, SHORT![T::SAMPLE_FORMAT as u16]) // int
            .with_entry(tags::ImageLength, LONG![height as u32])
            .with_entry(tags::ImageWidth, LONG![width as u32])
            .with_entry(tags::ResolutionUnit, SHORT![1]) // No resolution unit
            .with_entry(tags::XResolution, RATIONAL![(1, 1)])
            .with_entry(tags::YResolution, RATIONAL![(1, 1)])
            .with_entry(tags::RowsPerStrip, LONG![height as u32]) // One strip for the whole image
            .with_entry(tags::StripByteCounts, LONG![image_bytes.len() as u32])
            .with_entry(tags::StripOffsets, ByteBlock::single(image_bytes))
            .single(),
    )
    .with_endianness(endianness.for_encoding())
    .write_to(filename)?;

    Ok(())
}
