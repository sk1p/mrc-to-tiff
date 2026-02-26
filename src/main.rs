mod convert;
mod read;
mod write;
mod render;
mod common;

use std::{error::Error, path::PathBuf};

use clap::Parser;
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;

use crate::common::ArgEndianess;

#[derive(Parser, Debug)]
struct Args {
    /// Path to the input .mrc file. Must be a 3D stack in 16bit format.
    mrc_path: PathBuf,

    /// Destination path, should be an existing directory.
    dest_path: PathBuf,

    /// Which frame number should be the first to include? Starts at 1.
    #[arg(short, long, default_value = "1")]
    start_at_frame: usize,

    /// Which frame number should be the last to include? Starts at 1.
    #[arg(short, long)]
    stop_at_frame: Option<usize>,

    /// The endianess of the tiff files that are written.
    #[arg(short, long, default_value = "big")]
    endianess: ArgEndianess,
}

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    let env = env_logger::Env::default().filter_or("RUST_LOG", "info");
    let logger = env_logger::Builder::from_env(env).build();
    let multi = MultiProgress::new();
    LogWrapper::new(multi.clone(), logger).try_init()?;

    let args = Args::parse();

    convert::convert(
        args.mrc_path,
        args.dest_path,
        args.endianess,
        args.start_at_frame,
        args.stop_at_frame,
        &multi,
        None,
    )?;

    Ok(())
}
