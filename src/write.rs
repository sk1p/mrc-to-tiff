use std::{error::Error, fs::File, path::Path};

use byteorder::{BigEndian, WriteBytesExt};
use tiff::encoder::{TiffEncoder, colortype};
use tiff_encoder::{LONG, RATIONAL, SHORT, TiffFile, ifd::{Ifd, tags}, write::ByteBlock};


pub fn write_tiff_native_endian(
    filename: &Path,
    data: &[i16],
    width: usize,
    height: usize,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let mut out_file = File::create_new(filename)?;
    let mut tiff = TiffEncoder::new(&mut out_file)?;
    tiff.write_image::<colortype::GrayI16>(width as u32, height as u32, data)?;
    Ok(())
}

pub fn write_tiff_big_endian(
    filename: &Path,
    data: &[i16],
    width: usize,
    height: usize,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let mut image_bytes: Vec<u8> = Vec::with_capacity(width * height * 2);
    for value in data.iter() {
        image_bytes.write_i16::<BigEndian>(*value)?;
    }

    TiffFile::new(
        Ifd::new()
            .with_entry(tags::PhotometricInterpretation, SHORT![1]) // Black is zero
            .with_entry(tags::Compression, SHORT![1]) // No compression

            .with_entry(tags::BitsPerSample, SHORT![16])
            .with_entry(tags::SamplesPerPixel, SHORT![1])
            .with_entry(tags::SampleFormat, SHORT![2]) // int

            .with_entry(tags::ImageLength, LONG![height as u32])
            .with_entry(tags::ImageWidth, LONG![width as u32])

            .with_entry(tags::ResolutionUnit, SHORT![1]) // No resolution unit
            .with_entry(tags::XResolution, RATIONAL![(1, 1)])
            .with_entry(tags::YResolution, RATIONAL![(1, 1)])

            .with_entry(tags::RowsPerStrip, LONG![height as u32]) // One strip for the whole image
            .with_entry(tags::StripByteCounts, LONG![image_bytes.len() as u32])
            .with_entry(tags::StripOffsets, ByteBlock::single(image_bytes))
            .single()
    ).with_endianness(tiff_encoder::write::Endianness::MM).write_to(filename)?;

    Ok(())
}
