use std::{error::Error, path::PathBuf};

use log::info;
use mrc::MrcMmap;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::{ArgEndianess, read::Volume3D, write::{write_tiff_big_endian, write_tiff_native_endian}};

pub fn convert(mrc_path: PathBuf, dest_path: PathBuf, endianess: ArgEndianess) -> Result<(), Box<dyn Error + Sync + Send>> {
    let data = MrcMmap::open(mrc_path)?;

    let (nx, ny, nz) = data.read_view()?.dimensions();
    info!("dimensions: {nz}x{ny}x{nx}");

    let view = data.read_view()?;

    let ints = view.view::<i16>()?;
    info!("len of slice: {}", ints.len());

    info!("endianess: {:?}", endianess);

    let volume = Volume3D::new(view)?;
    let res: Result<Vec<()>, _> = (0..nz)
        .into_par_iter()
        .map(|z| -> Result<(), Box<dyn Error + Sync + Send>> {
            let slice = volume.get_slice(z)?;
            let out_path = dest_path.join(format!("slice_{z:05}.tif"));
            match endianess {
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