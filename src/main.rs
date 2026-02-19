mod write;
mod convert;
mod read;

use std::{
    error::Error,
    path::PathBuf,
};

use clap::{Parser};

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

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    let env = env_logger::Env::default().filter_or("RUST_LOG", "info");
    env_logger::init_from_env(env);

    let args = Args::parse();

    convert::convert(args.mrc_path, args.dest_path, args.endianess)?;

    Ok(())
}
