use std::{error::Error, path::{Path, PathBuf}};

use clap::Parser;
use eframe::egui::{self, Slider, TextureId, vec2};
use egui_plot::{Plot, PlotImage, PlotPoint};
use log::error;
use mrc::MrcMmap;

#[derive(Parser, Debug)]
struct Args {
    /// Path to the input .mrc file. Must be a 3D stack in 16bit format.
    mrc_path: Option<PathBuf>,
}

#[derive(Default, Debug)]
struct ConverterApp {
    dest_directory: Option<PathBuf>,
    input_data: Option<WithInputData>,
}

#[derive(Debug)]
struct WithInputData {
    source_path: PathBuf,
    mmap: MrcMmap,
    slice_position: usize,
    range_start: usize,
    range_end: usize,
}

fn load_data(path: &Path) -> Result<WithInputData, Box<dyn Error>> {
    let mmap = MrcMmap::open(&path)?;
    let view = mmap.read_view()?;
    Ok(WithInputData {
        source_path: path.to_owned(),
        range_start: 0,
        slice_position: 0,
        range_end: view.dimensions().2,
        mmap,
    })
}

fn main() -> Result<(), Box<dyn Error>> {
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
            };
            Ok(Box::new(app))
        }),
    )?;

    Ok(())
}

impl eframe::App for ConverterApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
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


                // 1-indexed position in the UI:
                let mut slider_value = data.slice_position + 1;
                ui.add(Slider::new(&mut slider_value, data.range_start + 1..=data.range_end));
                data.slice_position = slider_value - 1;

                let plot = Plot::new("preview").data_aspect(1.0);

                plot.show(ui, |plot_ui| {
                    // let texture_id = load_texture(data);
                    // // XXX we can't do this actually... must do it similarly to our demo thingy
                    // ui.ctx().load_texture(name, image, options);
                    // let center_position = PlotPoint::new(0.5, 0.5);
                    // let aspect_ratio = ny as f32 / nx as f32;
                    // let image = PlotImage::new("preview_image", texture_id, center_position, vec2(aspect_ratio, 1.0));
                    // plot_ui.image(image);
                });
            }
        });
    }
}


impl ConverterApp {
    fn load_texture(&mut self) {
        todo!();
        match self.input_data {
            Some(data) => {}
            None => {

            }
        }
    }
}