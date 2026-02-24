use std::{
    error::Error,
    path::{Path, PathBuf},
};

use clap::Parser;
use eframe::egui::{self, DragValue, Slider, vec2};
use egui_plot::{Plot, PlotImage, PlotPoint};
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use log::info;
use mrc::MrcMmap;

use crate::{read::Volume3D, render::render_to_rgb};
mod common;
mod convert;
mod read;
mod render;
mod write;

#[derive(Parser, Debug)]
struct Args {
    /// Path to the input .mrc file. Must be a 3D stack in 16bit format.
    mrc_path: Option<PathBuf>,
}

#[derive(Default)]
struct ConverterApp {
    dest_directory: Option<PathBuf>,
    input_data: Option<WithInputData>,
    quantile: f32,
}

struct WithInputData {
    source_path: PathBuf,
    mmap: MrcMmap,
    slice_position: usize,
    range_start: usize,
    range_end: usize,

    export_start: usize,
    export_end: usize,

    texture: Option<egui::TextureHandle>,
}

fn load_data(path: &Path) -> Result<WithInputData, Box<dyn Error>> {
    let mmap = MrcMmap::open(path)?;
    let view = mmap.read_view()?;
    let range_end = view.dimensions().2;
    Ok(WithInputData {
        source_path: path.to_owned(),
        range_start: 0,
        slice_position: 0,
        range_end,
        mmap,
        texture: None,
        export_start: 0,
        export_end: range_end,
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    let env = env_logger::Env::default().filter_or("RUST_LOG", "info");
    let logger = env_logger::Builder::from_env(env).build();
    let multi = MultiProgress::new();
    LogWrapper::new(multi.clone(), logger).try_init()?;

    let args = Args::parse();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1024.0, 1024.0]),
        ..Default::default()
    };

    eframe::run_native(
        "MRC to TIFF converter",
        options,
        Box::new(|_cc| {
            let input_data = match args.mrc_path {
                Some(path) => load_data(&path).unwrap(),
                None => todo!(),
            };
            let app = ConverterApp {
                dest_directory: None,
                input_data: Some(input_data),
                quantile: 0.999,
            };
            Ok(Box::new(app))
        }),
    )?;

    Ok(())
}

impl eframe::App for ConverterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // egui::SidePanel::new(Side::Right, "view options").show(ctx, |ui| {
        // });
        egui::TopBottomPanel::new(
            egui::panel::TopBottomSide::Bottom,
            "bottom panel view options",
        )
        .show(ctx, |ui| {
            if let Some(data) = &mut self.input_data {
                ui.set_min_width(256.0);
                ui.label("Slice preview:");
                // 1-indexed position in the UI:
                let mut slider_value = data.slice_position + 1;
                ui.add(Slider::new(
                    &mut slider_value,
                    data.range_start + 1..=data.range_end,
                ));
                let new_slice_position = slider_value - 1;
                // slider change detected:
                if data.slice_position != new_slice_position {
                    data.texture = None;
                }
                data.slice_position = new_slice_position;

                ui.label("Quantile");
                let mut slider_quantile = self.quantile;
                let q_slider = Slider::new(&mut slider_quantile, 0.0..=1.0);
                ui.add(q_slider);
                if self.quantile != slider_quantile {
                    data.texture = None;
                }
                self.quantile = slider_quantile;
            }
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            if ui.button("Load...").clicked()
                && let Some(new_path) = rfd::FileDialog::new().pick_file()
            {
                self.input_data = Some(load_data(&new_path).unwrap());
            }
            if let Some(data) = &mut self.input_data {
                let view = data.mmap.read_view().unwrap();
                let (nx, ny, nz) = view.dimensions();
                ui.label("Input path: ");
                ui.monospace(data.source_path.to_string_lossy());
                ui.label(format!("Input size: {nz}x{ny}x{nx}"));

                if ui.button("Destination directory...").clicked()
                    && let Some(new_path) = rfd::FileDialog::new().pick_folder()
                {
                    self.dest_directory = Some(new_path);
                }

                if let Some(dest_path) = &self.dest_directory {
                    ui.label("Destination directory:");
                    ui.monospace(dest_path.to_string_lossy());
                }

                if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                    data.slice_position = data.slice_position.saturating_sub(1);
                    data.texture = None;
                };

                if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                    data.slice_position = data
                        .slice_position
                        .saturating_add(1)
                        .min(data.range_end - 1);
                    data.texture = None;
                };

                ui.label("Start frame number:");
                let mut export_start_drag = data.export_start + 1;
                ui.add(DragValue::new(&mut export_start_drag).range(1..=data.export_end));
                data.export_start = export_start_drag - 1;

                ui.label("End frame number (inclusive):");
                let mut export_end_drag = data.export_end + 1;
                ui.add(DragValue::new(&mut export_end_drag).range(1..=data.range_end));
                data.export_end = export_end_drag - 1;

                let multi_progress=  Default::default();

                if let Some(dest_directory) = &self.dest_directory {
                    if ui.button("Export to tiff").clicked() {
                        convert::convert(
                            data.source_path.clone(),
                            dest_directory.clone(),
                            common::ArgEndianess::Big,
                            data.export_start,
                            Some(data.export_end),
                            &multi_progress,
                        ).unwrap();
                    }
                }

                let texture: &egui::TextureHandle = data.texture.get_or_insert_with(|| {
                    let view = data.mmap.read_view().unwrap();
                    let volume = Volume3D::new(view).unwrap();
                    info!("loading slice {}", data.slice_position);
                    let img = render_to_rgb(
                        volume.get_slice(data.slice_position).unwrap(),
                        nx,
                        ny,
                        self.quantile,
                    );
                    ui.ctx()
                        .load_texture("preview_texture", img, Default::default())
                });
                let plot = Plot::new("preview").data_aspect(1.0);
                plot.show(ui, |plot_ui| {
                    let center_position = PlotPoint::new(0.5, 0.5);
                    let aspect_ratio = ny as f32 / nx as f32;
                    let image = PlotImage::new(
                        "preview_image",
                        texture,
                        center_position,
                        vec2(aspect_ratio, 1.0),
                    );
                    plot_ui.image(image);
                });
            }
        });
    }
}
