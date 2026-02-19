use std::{error::Error, fs::File, path::PathBuf};

use clap::Parser;
use log::info;
use mrc::{MrcMmap, MrcView};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use tiff::encoder::{TiffEncoder, colortype};

#[derive(Parser, Debug)]
struct Args {
    mrc_path: PathBuf,
    dest_path: PathBuf,
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

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    let env = env_logger::Env::default()
        .filter_or("RUST_LOG", "info");
    env_logger::init_from_env(env);

    let args = Args::parse();

    let data = MrcMmap::open(args.mrc_path)?;

    let (nx, ny, nz) = data.read_view()?.dimensions();
    info!("dimensions: {nz}x{ny}x{nx}");

    let view = data.read_view()?;

    let ints = view.view::<i16>()?;
    info!("len of slice: {}", ints.len());

    let volume = Volume3D::new(view)?;
    let res: Result<Vec<()>, _> = (0..nz)
        .into_par_iter()
        .map(|z| -> Result<(), Box<dyn Error + Sync + Send>> {
            let slice = volume.get_slice(z)?;
            let out_path = args.dest_path.join(format!("slice_{z:05}.tif"));
            let mut out_file = File::create_new(&out_path)?;
            let mut tiff = TiffEncoder::new(&mut out_file)?;
            tiff.write_image::<colortype::GrayI16>(nx as u32, ny as u32, slice)?;
            info!("created {out_path:?}");
            Ok(())
        })
        .collect();
    res?;

    Ok(())
}
