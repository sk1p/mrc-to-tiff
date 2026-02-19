use std::{
    error::Error,
    fs::File,
    path::{Path, PathBuf},
};

use byteorder::{BigEndian, LittleEndian, WriteBytesExt};
use clap::{Parser};
use log::info;
use mrc::{MrcMmap, MrcView};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use tiff::encoder::{TiffEncoder, colortype};
use tiff_encoder::{LONG, RATIONAL, SHORT, TiffFile, ifd::{Ifd, tags}, write::ByteBlock};

#[derive(Debug, clap::ValueEnum, Clone)]
enum ArgEndianess {
    Big,
    Native,
}

#[derive(Parser, Debug)]
struct Args {
    mrc_path: PathBuf,
    dest_path: PathBuf,
    #[arg(default_value = "big")]
    endianess: ArgEndianess,
}

// adapted from the docs of the mrc crate
struct Volume3D<'a> {
    view: MrcView<'a>,
    nx: usize,
    ny: usize,
    nz: usize,
}

impl<'a> Volume3D<'a> {
    fn new(view: MrcView<'a>) -> Result<Self, mrc::Error> {
        let (nx, ny, nz) = view.dimensions();
        Ok(Self { view, nx, ny, nz })
    }

    fn get_slice(&self, z: usize) -> Result<&[i16], mrc::Error> {
        if z >= self.nz {
            return Err(mrc::Error::InvalidDimensions);
        }

        let slice_size = self.nx * self.ny;
        let start = z * slice_size;
        let ints = self.view.view::<i16>()?;

        ints.get(start..start + slice_size)
            .ok_or(mrc::Error::InvalidDimensions)
    }
}

fn write_tiff_native_endian(
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

fn write_tiff_big_endian(
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

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    let env = env_logger::Env::default().filter_or("RUST_LOG", "info");
    env_logger::init_from_env(env);

    let args = Args::parse();

    let data = MrcMmap::open(args.mrc_path)?;

    let (nx, ny, nz) = data.read_view()?.dimensions();
    info!("dimensions: {nz}x{ny}x{nx}");

    let view = data.read_view()?;

    let ints = view.view::<i16>()?;
    info!("len of slice: {}", ints.len());

    info!("endianess: {:?}", args.endianess);

    let volume = Volume3D::new(view)?;
    let res: Result<Vec<()>, _> = (0..nz)
        .into_par_iter()
        .map(|z| -> Result<(), Box<dyn Error + Sync + Send>> {
            let slice = volume.get_slice(z)?;
            let out_path = args.dest_path.join(format!("slice_{z:05}.tif"));
            match args.endianess {
                ArgEndianess::Big => {
                    write_tiff_big_endian(&out_path, slice, nx, ny)?;
                }
                ArgEndianess::Native => {
                    write_tiff_native_endian(&out_path, slice, nx, ny)?;
                }
            }
            info!("created {out_path:?}");
            Ok(())
        })
        .collect();
    res?;

    Ok(())
}
