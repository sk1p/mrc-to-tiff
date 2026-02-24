use std::{error::Error, path::PathBuf, time::Instant};

use indicatif::{MultiProgress, ParallelProgressIterator, ProgressBar};
use log::{debug, info};
use mrc::MrcMmap;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::{
    common::ArgEndianess, read::Volume3D, write::{write_tiff_big_endian, write_tiff_native_endian}
};

pub fn convert(
    mrc_path: PathBuf,  // 3d, 16bit
    dest_path: PathBuf,  // directory
    endianess: ArgEndianess,  // tif output endianess
    start_at_frame: usize,  // 1-indexed
    stop_at_frame: Option<usize>, // 1-indexed, last frame if not given
    multi_progress: &MultiProgress,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let t0 = Instant::now();

    let data = MrcMmap::open(mrc_path)?;

    let (nx, ny, nz) = data.read_view()?.dimensions();
    info!("dimensions: {nz}x{ny}x{nx}");

    let view = data.read_view()?;

    let ints = view.data.as_i16_slice()?;
    debug!("len of slice: {}", ints.len());

    info!("endianess: {:?}", endianess);

    let start = start_at_frame - 1;
    let stop = stop_at_frame.unwrap_or(nz);

    assert!(start <= stop);

    let volume = Volume3D::new(view)?;
    let idxs: Vec<usize> = (start..stop).collect();
    let len = idxs.len() as u64;
    let progress = multi_progress.add(ProgressBar::new(len));

    let res: Result<Vec<()>, _> = idxs
        .into_par_iter()
        .progress_with(progress.clone())
        .map(|z| -> Result<(), Box<dyn Error + Sync + Send>> {
            let slice = volume.get_slice(z)?;
            let idx = z + 1 - start;
            let out_path = dest_path.join(format!("slice_{idx:05}.tif"));
            match endianess {
                ArgEndianess::Big => {
                    write_tiff_big_endian(&out_path, slice, nx, ny)?;
                }
                ArgEndianess::Native => {
                    write_tiff_native_endian(&out_path, slice, nx, ny)?;
                }
            }
            debug!("created {out_path:?}");
            Ok(())
        })
        .collect();
    res?;

    progress.finish();
    multi_progress.remove(&progress);

    info!("conversion done in {:?}", t0.elapsed());

    Ok(())
}
