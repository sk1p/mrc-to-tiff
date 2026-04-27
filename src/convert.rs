use std::{
    error::Error,
    path::PathBuf,
    sync::{
        Arc, atomic::{AtomicUsize, Ordering}, mpsc::Sender
    },
    time::Instant,
};

use indicatif::{MultiProgress, ParallelProgressIterator, ProgressBar};
use log::{debug, info};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::{
    common::OutputEndianess, datasource::{DataSource, load_any}, write::write_tiff
};

#[derive(Debug)]
pub enum ProgressMessage {
    InProgress { num_done: usize, total: usize },
    Done { total: usize },
    Error { msg: String },
}

pub fn convert(
    data: Arc<Box<dyn DataSource>>,    // 3d, 16bit
    dest_path: PathBuf,           // directory
    endianess: OutputEndianess,      // tif output endianess
    start_at_frame: usize,        // 1-indexed
    stop_at_frame: Option<usize>, // 1-indexed, last frame if not given
    multi_progress: &MultiProgress,
    progress_q: Option<Sender<ProgressMessage>>,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let t0 = Instant::now();

    let (nx, ny, nz) = data.dimensions();
    info!("dimensions: {nz}x{ny}x{nx}");

    info!("endianess: {:?}", endianess);

    let start = start_at_frame - 1;
    let stop = stop_at_frame.unwrap_or(nz);

    assert!(start <= stop);

    let idxs: Vec<usize> = (start..stop).collect();
    let len = idxs.len() as u64;
    let progress = multi_progress.add(ProgressBar::new(len));

    // alternative "progress bar" for GUI version
    let done = AtomicUsize::new(0);

    let res: Result<Vec<()>, _> = idxs
        .into_par_iter()
        .progress_with(progress.clone())
        .map(|z| -> Result<(), Box<dyn Error + Sync + Send>> {
            let slice = data.get_slice(z);
            let idx = z + 1 - start;
            let out_path = dest_path.join(format!("slice_{idx:05}.tif"));
            write_tiff(&out_path, &slice, nx, ny, endianess)?;
            done.fetch_add(1, Ordering::SeqCst);
            if let Some(prog_q) = &progress_q {
                prog_q
                    .send(ProgressMessage::InProgress {
                        num_done: done.load(Ordering::SeqCst),
                        total: len as usize,
                    })?;
            }
            debug!("created {out_path:?}");
            Ok(())
        })
        .collect();
    res?;

    progress.finish();
    if let Some(prog_q) = &progress_q {
        prog_q
            .send(ProgressMessage::Done {
                total: len as usize,
            })
            .unwrap();
    }
    multi_progress.remove(&progress);

    info!("conversion done in {:?}", t0.elapsed());

    Ok(())
}
