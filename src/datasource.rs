use std::{borrow::Cow, error::Error, path::Path};

use dm3dm4::dataset::{DMArray, DMDataSet};
use log::info;
use mrc::MmapReader;
use zerocopy::FromBytes;

pub struct MrcDataSource {
    pub src: MmapReader,
}

pub struct DmDataSource {
    pub src: DMArray,
}

pub trait DataSource: Sync + Send {
    fn dimensions(&self) -> (usize, usize, usize);
    fn get_slice<'a>(&'a self, z: usize) -> Cow<'a, [i16]>;
    fn get_slice_f32(&self, z: usize) -> Cow<'_, [f32]>;
}

impl DataSource for MrcDataSource {
    fn dimensions(&self) -> (usize, usize, usize) {
        let s = self.src.shape();
        (s.nx, s.ny, s.nz)
    }

    fn get_slice(&self, z: usize) -> Cow<'_, [i16]> {
        let src = &self.src;
        let shape = src.shape();
        let bytes = src.data_bytes();

        let slice_size_bytes = shape.nx * shape.ny * src.mode().byte_size();
        let start_bytes = z * slice_size_bytes;
        let bytes_slice = &bytes[start_bytes..start_bytes + slice_size_bytes];

        if src.can_zero_copy::<i16>() {
            let zslice: &[i16] = FromBytes::ref_from_bytes(bytes_slice).unwrap();
            Cow::Borrowed(zslice)
        } else {
            let offset = [0, 0, z];
            let shape = [shape.nx, shape.ny, 1];
            let block = match src.mode() {
                mrc::Mode::Int8 => src.read_converted::<i8, i16>(offset, shape),
                mrc::Mode::Int16 => src.read_converted::<i16, i16>(offset, shape),
                mrc::Mode::Float32 => src.read_converted::<f32, i16>(offset, shape),
                mrc::Mode::Uint16 => src.read_converted::<u16, i16>(offset, shape),
                // mrc::Mode::Float16 => todo!(),
                // mrc::Mode::Packed4Bit => todo!(),
                // mrc::Mode::Int16Complex => todo!(),
                // mrc::Mode::Float32Complex => todo!(),
                _ => todo!(),
            }
            .unwrap();

            Cow::Owned(block.data)
        }
    }

    fn get_slice_f32(&self, z: usize) -> Cow<'_, [f32]> {
        let src = &self.src;
        let shape = src.shape();
        let bytes = src.data_bytes();

        let slice_size_bytes = shape.nx * shape.ny * src.mode().byte_size();
        let start_bytes = z * slice_size_bytes;
        let bytes_slice = &bytes[start_bytes..start_bytes + slice_size_bytes];

        if src.can_zero_copy::<f32>() {
            let zslice: &[f32] = FromBytes::ref_from_bytes(bytes_slice).unwrap();
            Cow::Borrowed(zslice)
        } else {
            let offset = [0, 0, z];
            let shape = [shape.nx, shape.ny, 1];
            let block = match src.mode() {
                mrc::Mode::Int8 => src.read_converted::<i8, f32>(offset, shape),
                mrc::Mode::Int16 => src.read_converted::<i16, f32>(offset, shape),
                mrc::Mode::Float32 => src.read_converted::<f32, f32>(offset, shape),
                mrc::Mode::Uint16 => src.read_converted::<u16, f32>(offset, shape),
                // mrc::Mode::Float16 => todo!(),
                // mrc::Mode::Packed4Bit => todo!(),
                // mrc::Mode::Int16Complex => todo!(),
                // mrc::Mode::Float32Complex => todo!(),
                _ => todo!(),
            }
            .unwrap();

            Cow::Owned(block.data)
        }
    }
}

impl DataSource for DmDataSource {
    fn dimensions(&self) -> (usize, usize, usize) {
        let raw_shape = self.src.shape();
        (raw_shape[0], raw_shape[1], raw_shape[2])
    }

    fn get_slice(&self, z: usize) -> Cow<'_, [i16]> {
        info!("dm endianess: {:?}", self.src.dataset.doc.header.byte_order);
        info!("get_slice for type {:?}", self.src.typ());

        match self.src.typ() {
            dm3dm4::parser::Type::Short => self.src.as_i16_zslice(z).unwrap(),
            dm3dm4::parser::Type::Long => Cow::Owned(
                self.src
                    .as_i32_zslice(z)
                    .unwrap()
                    .iter()
                    .copied()
                    .map(|i| i as i16)
                    .collect::<Vec<_>>(),
            ),
            dm3dm4::parser::Type::UShort => Cow::Owned(
                self.src
                    .as_u16_zslice(z)
                    .unwrap()
                    .iter()
                    .copied()
                    .map(|i| i as i16)
                    .collect(),
            ),
            dm3dm4::parser::Type::ULong => Cow::Owned(
                self.src
                    .as_u32_zslice(z)
                    .unwrap()
                    .iter()
                    .copied()
                    .map(|i| i as i16)
                    .collect(),
            ),
            dm3dm4::parser::Type::Float => Cow::Owned(
                self.src
                    .as_f32_zslice(z)
                    .unwrap()
                    .iter()
                    .copied()
                    .map(|i| i as i16)
                    .collect(),
            ),
            dm3dm4::parser::Type::Double => Cow::Owned(
                self.src
                    .as_f64_zslice(z)
                    .unwrap()
                    .iter()
                    .copied()
                    .map(|i| i as i16)
                    .collect(),
            ),
            dm3dm4::parser::Type::Boolean => todo!(),
            dm3dm4::parser::Type::Char => todo!(),
            dm3dm4::parser::Type::Octet => todo!(),
            dm3dm4::parser::Type::LongLong => todo!(),
            dm3dm4::parser::Type::ULongLong => todo!(),
            dm3dm4::parser::Type::Struct => todo!(),
            dm3dm4::parser::Type::String => todo!(),
            dm3dm4::parser::Type::Array => todo!(),
        }
    }

    fn get_slice_f32(&self, z: usize) -> Cow<'_, [f32]> {
        info!("dm endianess: {:?}", self.src.dataset.doc.header.byte_order);
        info!("get_slice for type {:?}", self.src.typ());

        let cow = match self.src.typ() {
            dm3dm4::parser::Type::Short => Cow::Owned(
                self.src
                    .as_i16_zslice(z)
                    .unwrap()
                    .iter()
                    .copied()
                    .map(|i| i as f32)
                    .collect::<Vec<_>>(),
            ),
            dm3dm4::parser::Type::Long => Cow::Owned(
                self.src
                    .as_i32_zslice(z)
                    .unwrap()
                    .iter()
                    .copied()
                    .map(|i| i as f32)
                    .collect::<Vec<_>>(),
            ),
            dm3dm4::parser::Type::UShort => Cow::Owned(
                self.src
                    .as_u16_zslice(z)
                    .unwrap()
                    .iter()
                    .copied()
                    .map(|i| i as f32)
                    .collect(),
            ),
            dm3dm4::parser::Type::ULong => Cow::Owned(
                self.src
                    .as_u32_zslice(z)
                    .unwrap()
                    .iter()
                    .copied()
                    .map(|i| i as f32)
                    .collect(),
            ),
            dm3dm4::parser::Type::Float => self.src.as_f32_zslice(z).unwrap(),
            dm3dm4::parser::Type::Double => Cow::Owned(
                self.src
                    .as_f64_zslice(z)
                    .unwrap()
                    .iter()
                    .copied()
                    .map(|i| i as f32)
                    .collect(),
            ),
            dm3dm4::parser::Type::Boolean => todo!(),
            dm3dm4::parser::Type::Char => todo!(),
            dm3dm4::parser::Type::Octet => todo!(),
            dm3dm4::parser::Type::LongLong => todo!(),
            dm3dm4::parser::Type::ULongLong => todo!(),
            dm3dm4::parser::Type::Struct => todo!(),
            dm3dm4::parser::Type::String => todo!(),
            dm3dm4::parser::Type::Array => todo!(),
        };
        match &cow {
            Cow::Borrowed(_) => info!("borrowed -> zero copy-ish"),
            Cow::Owned(_) => info!("owned -> needs copy for decoding"),
        };
        cow
    }
}

pub fn load_any(path: &Path) -> Result<Box<dyn DataSource>, Box<dyn Error + Sync + Send>> {
    let ext = path.extension().unwrap().to_str().unwrap().to_lowercase();
    let data: Box<dyn DataSource> = if ext == "dm3" || ext == "dm4" {
        let ds = DMDataSet::load(path)?;
        let arrs = ds.arrays()?;
        let arr = arrs.first().unwrap();
        Box::new(DmDataSource { src: arr.clone() })
    } else {
        Box::new(MrcDataSource {
            src: MmapReader::open(path.to_str().unwrap())?,
        })
    };
    Ok(data)
}
