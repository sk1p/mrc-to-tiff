mod common;
mod convert;
mod read;
mod render;
mod write;
mod datasource;

use std::{
    error::Error,
    io::{Cursor, Seek},
    path::{Path, PathBuf}, sync::Arc,
};

use byteorder::{LittleEndian, ReadBytesExt};
use clap::Parser;
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use log::{debug, info, warn};
use mrc::MmapReader;

use crate::{common::OutputEndianess, datasource::load_any};

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
    endianess: OutputEndianess,

    /// Only output information about the MRC input file
    #[arg(short, long)]
    info_only: bool,
}

fn string_from_header(
    extra_header: &[u8],
    offset: usize,
    len: usize,
) -> Result<String, Box<dyn Error + Sync + Send>> {
    Ok(String::from_utf8(extra_header[offset..offset + len].to_vec())?)
}

fn dump_mrc_info(path: &Path) -> Result<(), Box<dyn Error + Sync + Send>> {
    info!("Loading {}...", path.to_string_lossy());

    let data = MmapReader::open(path.to_str().unwrap())?;

    let header = data.header();

    let exttyp = header.exttyp_str()?;

    let shape = data.shape();
    let (nx, ny, nz) = (shape.nx, shape.ny, shape.nz);

    info!("dimensions: {nz}x{ny}x{nx}");
    info!("header: {header:?}");
    info!("exttyp: {exttyp}");
    info!("machst: {:?}", header.machst);

    match exttyp {
        "FEI1" => {
            warn!("support for reading FEI1 extra header not implemented");
        }
        "FEI2" => {
            todo!();
            let extra_header_raw = data.ext_header();
            info!("ext header size: {}", extra_header_raw.len());

            let mut cursor = Cursor::new(extra_header_raw);

            let _metadata_size = cursor.read_i32::<LittleEndian>()?;
            let _metadata_version = cursor.read_i32::<LittleEndian>()?;
            let _bitmask_1 = cursor.read_u32::<LittleEndian>()?;
            let _timestamp = cursor.read_f64::<LittleEndian>()?;

            let mic_type = string_from_header(extra_header_raw, 20, 16)?;
            debug!("mic_type raw: {:?}", &extra_header_raw[20..36]);

            // TODO: implement other version-specific fields?
            // TODO: make some of these Option<T> based on the bitmask

            // continue with "gun" fields
            cursor.seek(std::io::SeekFrom::Start(84))?;
            let ht_volt = cursor.read_f64::<LittleEndian>()?;
            let dose_e_per_sqm = cursor.read_f64::<LittleEndian>()?;

            // stage
            // general tilts
            let alpha_tilt_deg = cursor.read_f64::<LittleEndian>()?;
            let beta_tilt_deg = cursor.read_f64::<LittleEndian>()?;
            // general stage position
            let x_stage_m = cursor.read_f64::<LittleEndian>()?;
            let y_stage_m = cursor.read_f64::<LittleEndian>()?;
            let z_stage_m = cursor.read_f64::<LittleEndian>()?;
            // "angle of tilt axis in image"
            let tilt_axis_angle_deg = cursor.read_f64::<LittleEndian>()?;
            // "measured rotation angle after b flip (tomography only)"
            let dual_axis_rot_deg = cursor.read_f64::<LittleEndian>()?;

            // making sure we're still on track...
            assert_eq!(cursor.position(), 156);

            // pixel sizes
            let pixel_size_x_m = cursor.read_f64::<LittleEndian>()?;
            let pixel_size_y_m = cursor.read_f64::<LittleEndian>()?;

            // optics
            cursor.seek(std::io::SeekFrom::Start(220))?;
            let defocus_m = cursor.read_f64::<LittleEndian>()?;
            let stem_defocus_m = cursor.read_f64::<LittleEndian>()?;
            let applied_defocus_m = cursor.read_f64::<LittleEndian>()?;
            let instrument_mode = cursor.read_i32::<LittleEndian>()?; // 1 -> TEM, 2 -> STEM
            let projection_mode = cursor.read_i32::<LittleEndian>()?; // 1 -> diffraction, 2-> imaging

            // LM, HM, Lorentz?
            let objective_lens_mode = string_from_header(extra_header_raw, 252, 16)?;

            // high mag mode: Mi, SA, Mh
            let high_mag_mode = string_from_header(extra_header_raw, 268, 16)?;

            // probe mode
            cursor.seek(std::io::SeekFrom::Start(284))?;
            let probe_mode = cursor.read_i32::<LittleEndian>()?; // 1 -> nano probe, 2 -> micro probe
            let eftem_on = cursor.read_u8()? == 1;
            let magnification = cursor.read_f64::<LittleEndian>()?;

            let _bitmask_2 = cursor.read_u32::<LittleEndian>()?;

            cursor.seek(std::io::SeekFrom::Start(490))?;
            let _bitmask_3 = cursor.read_u32::<LittleEndian>()?;

            cursor.seek(std::io::SeekFrom::Start(748))?;
            let _bitmask_4 = cursor.read_u32::<LittleEndian>()?;

            info!("bitmasks: {_bitmask_1:08x} {_bitmask_2:08x} {_bitmask_3:08x} {_bitmask_4:08x}");

            // TODO: more fields here...

            let cam_name = string_from_header(extra_header_raw, 435, 16)?;

            // some interesting fields maybe?
            cursor.seek(std::io::SeekFrom::Start(820))?;

            let start_tilt_angle_deg = cursor.read_f64::<LittleEndian>()?;
            let end_tilt_angle_deg = cursor.read_f64::<LittleEndian>()?;
            let tilt_per_image_deg = cursor.read_f64::<LittleEndian>()?;
            let tilt_speed_deg_s = cursor.read_f64::<LittleEndian>()?;

            let cam_name_com = string_from_header(extra_header_raw, 804, 16)?;

            info!("camera name: {cam_name_com}");
            info!("mic name: {mic_type}");
            info!("ht: {ht_volt}V");
            info!("dose: {dose_e_per_sqm}e/m²");
            info!("obj lens mode: {objective_lens_mode}");
            info!("high mag mode: {high_mag_mode}");

            info!(
                "stage: alpha={alpha_tilt_deg}deg beta={beta_tilt_deg}deg"
            );
            info!(
                "    x={x_stage_m}m y={y_stage_m}m z={z_stage_m}"
            );
            info!(
                "    tilt_axis_angle={tilt_axis_angle_deg}deg"
            );
            info!(
                "    dual_axis_rot={dual_axis_rot_deg}deg"
            );

            info!("pixel size: {pixel_size_x_m}x{pixel_size_y_m}");

            info!("defocus: {defocus_m}m");
            info!("STEM defocus: {stem_defocus_m}m");
            info!("applied defocus: {applied_defocus_m}m");
            info!("instrument mode: {instrument_mode}");
            info!("projection mode: {projection_mode}");
            info!("probe mode: {probe_mode}");
            info!("EFTEM? {eftem_on}");
            info!("magnification: {magnification}");

            info!("start tilt angle: {start_tilt_angle_deg}deg");
            info!("end tilt angle: {end_tilt_angle_deg}deg");
            info!("tilt per image: {tilt_per_image_deg}deg");
            info!("tilt speed: {tilt_speed_deg_s}deg/s");
        },
        typ => {
            warn!("unknown ext header type: {typ}");
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    let env = env_logger::Env::default().filter_or("RUST_LOG", "info");
    let logger = env_logger::Builder::from_env(env).build();
    let multi = MultiProgress::new();
    LogWrapper::new(multi.clone(), logger).try_init()?;

    let args = Args::parse();

    if args.info_only {
        dump_mrc_info(&args.mrc_path)?;
    } else {
        let input_data = Arc::new(load_any(&args.mrc_path)?);
        convert::convert(
            Arc::clone(&input_data),
            args.dest_path,
            args.endianess,
            args.start_at_frame,
            args.stop_at_frame,
            &multi,
            None,
        )?;
    }

    Ok(())
}
