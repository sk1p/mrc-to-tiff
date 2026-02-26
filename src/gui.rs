use std::{
    error::Error,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    time::Duration,
};

use clap::Parser;
use eframe::egui::{self, DragValue, RichText, Slider, Spacing, Style, vec2};
use egui_plot::{Plot, PlotImage, PlotPoint};
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use log::{error, info};
use mrc::MrcMmap;

use crate::{convert::ProgressMessage, read::Volume3D, render::render_to_rgb};
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

const H: f32 = 15.0;
const V: f32 = 10.0;

#[derive(Default)]
struct ConverterApp {
    dest_directory: Option<PathBuf>,
    input_data: Option<WithInputData>,
    quantile: f32,
    multi: MultiProgress,
    error_state: Option<String>,
}

#[derive(Debug)]
struct BgProgress {
    done: usize,
    total: usize,
}

struct WithInputData {
    source_path: PathBuf,
    mmap: MrcMmap,
    slice_position: usize,
    num_frames: usize,

    export_start: usize,
    export_end: usize,

    texture: Option<egui::TextureHandle>,

    // data for tracking the ongoing export operation (running in a background thread)
    background_progress: Option<Receiver<ProgressMessage>>,
    background_progress_nums: Option<BgProgress>,
}

fn load_data(path: &Path) -> Result<WithInputData, Box<dyn Error>> {
    let mmap = MrcMmap::open(path)?;
    let view = mmap.read_view()?;
    let num_frames = view.dimensions().2;
    Ok(WithInputData {
        source_path: path.to_owned(),
        slice_position: 0,
        num_frames,
        mmap,
        texture: None,
        export_start: 0,
        export_end: num_frames,
        background_progress: None,
        background_progress_nums: None,
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
            let input_data = args.mrc_path.map(|path| load_data(&path).unwrap());
            let app = ConverterApp {
                dest_directory: None,
                input_data,
                quantile: 0.999,
                multi,
                error_state: None,
            };
            Ok(Box::new(app))
        }),
    )?;

    Ok(())
}

impl eframe::App for ConverterApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let mut style = Style::default();
        let spacing = Spacing {
            button_padding: vec2(H, V),
            item_spacing: vec2(H, V),
            ..Spacing::default()
        };
        style.spacing = spacing;
        ctx.set_style_of(egui::Theme::Dark, style.clone());
        ctx.set_style_of(egui::Theme::Light, style);

        if let Some(err) = self.error_state.clone() {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.label(err.to_string());

                    let continue_btn = egui::Button::new(RichText::new("Continue").strong());
                    let continue_btn = continue_btn.fill(egui::Color32::from_rgb(0, 90, 230));

                    if ui.add(continue_btn).clicked() {
                        self.error_state = None;
                    }
                });
            });
        } else {
            if self.input_data.is_some() {
                self.render_with_data(ctx, frame);
            } else {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                        let load_btn =
                            egui::Button::new(RichText::new("Load 3D MRC stack...").strong());
                        let load_btn = load_btn.fill(egui::Color32::from_rgb(0, 90, 230));

                        if ui.add(load_btn).clicked()
                            && let Some(new_path) = rfd::FileDialog::new().pick_file()
                        {
                            self.input_data = match load_data(&new_path) {
                                Ok(data) => Some(data),
                                Err(err) => {
                                    self.error_state = Some(format!("Error loading data: {}", err));
                                    None
                                }
                            }
                        }
                    })
                });
            }
        }
    }
}

impl ConverterApp {
    fn render_with_data(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::new(
            egui::panel::TopBottomSide::Bottom,
            "bottom panel view options",
        )
        .frame(egui::containers::Frame::new().inner_margin(vec2(H, V)))
        .show(ctx, |ui| {
            if let Some(data) = &mut self.input_data {
                ui.set_min_width(256.0);
                // 1-indexed position in the UI:
                let mut slider_value = data.slice_position + 1;
                ui.add(
                    Slider::new(&mut slider_value, 1..=data.num_frames)
                        .text("Slice preview")
                        .drag_value_speed(0.1),
                );
                let new_slice_position = slider_value - 1;
                // slider change detected:
                if data.slice_position != new_slice_position {
                    data.texture = None;
                }
                data.slice_position = new_slice_position;

                let mut slider_quantile = self.quantile;
                let q_slider = Slider::new(&mut slider_quantile, 0.0..=1.0)
                    .text("Quantile")
                    .drag_value_speed(0.0001);
                ui.add(q_slider);
                if self.quantile != slider_quantile {
                    data.texture = None;
                }
                self.quantile = slider_quantile;
            }
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            if ui.button("Load 3D MRC Stack...").clicked()
                && let Some(new_path) = rfd::FileDialog::new().pick_file()
            {
                self.input_data = match load_data(&new_path) {
                    Ok(data) => Some(data),
                    Err(err) => {
                        self.error_state = Some(err.to_string());
                        None
                    }
                }
            }
            egui::Grid::new("parameter grid")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    if let Some(data) = &mut self.input_data {
                        let view = data.mmap.read_view().unwrap();
                        let (nx, ny, nz) = view.dimensions();
                        ui.label("Input path");
                        ui.monospace(data.source_path.to_string_lossy());
                        ui.end_row();
                        ui.label("Input size");
                        ui.monospace(format!("{nz}x{ny}x{nx}"));
                        ui.end_row();

                        ui.separator();
                        ui.separator();
                        ui.end_row();

                        let dest_btn =
                            egui::Button::new(RichText::new("Destination directory...").strong());
                        let dest_btn = dest_btn.fill(egui::Color32::from_rgb(0, 90, 230));

                        if ui.add(dest_btn).clicked()
                            && let Some(new_path) = rfd::FileDialog::new().pick_folder()
                        {
                            self.dest_directory = Some(new_path);
                        }
                        ui.end_row();

                        ui.label("Destination directory");
                        if let Some(dest_path) = &self.dest_directory {
                            ui.monospace(dest_path.to_string_lossy());
                        } else {
                            ui.label(RichText::new("not set").italics());
                        }
                        ui.end_row();

                        if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                            data.slice_position = data.slice_position.saturating_sub(1);
                            data.texture = None;
                        };
                        if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                            data.slice_position = data
                                .slice_position
                                .saturating_add(1)
                                .min(data.num_frames - 1);
                            data.texture = None;
                        };

                        ui.separator();
                        ui.separator();
                        ui.end_row();

                        ui.label("Start frame number");
                        ui.horizontal(|ui| {
                            let mut export_start_drag = data.export_start + 1;
                            ui.add(
                                DragValue::new(&mut export_start_drag).range(1..=data.export_end),
                            );
                            data.export_start = export_start_drag - 1;

                            if ui
                                .button(format!(
                                    "from current preview ({})",
                                    data.slice_position + 1
                                ))
                                .clicked()
                            {
                                data.export_start = data.slice_position;
                            }
                        });
                        ui.end_row();

                        ui.label("End frame number (inclusive)");
                        ui.horizontal(|ui| {
                            let mut export_end_drag = data.export_end + 1;
                            ui.add(DragValue::new(&mut export_end_drag).range(1..=data.num_frames));
                            data.export_end = export_end_drag - 1;
                            if ui
                                .button(format!(
                                    "from current preview ({})",
                                    data.slice_position + 1
                                ))
                                .clicked()
                            {
                                data.export_end = data.slice_position;
                            }
                        });
                        ui.end_row();

                        let export_enabled =
                            self.dest_directory.is_some() && data.background_progress.is_none();
                        let multi_progress = self.multi.clone();
                        ui.add_enabled_ui(export_enabled, |ui| {
                            let export_btn =
                                egui::Button::new(RichText::new("Export to tiff").strong());
                            let export_btn = export_btn.fill(egui::Color32::from_rgb(0, 90, 230));
                            let mut export_btn_resp = ui.add(export_btn);
                            if self.dest_directory.is_none() {
                                export_btn_resp = export_btn_resp
                                    .on_hover_text("Please select a destination directory first");
                            }
                            if export_btn_resp.clicked()
                                && let Some(dest_directory) = &self.dest_directory
                            {
                                info!(
                                    "converting frames {} to {} to tiff...",
                                    data.export_start + 1,
                                    data.export_end + 1
                                );
                                let (snd, rcv) = mpsc::channel::<ProgressMessage>();
                                data.background_progress = Some(rcv);

                                let source_path = data.source_path.clone();
                                let dest_directory = dest_directory.clone();
                                let export_start = data.export_start;
                                let export_end = data.export_end;

                                std::thread::spawn(move || {
                                    if let Err(e) = convert::convert(
                                        source_path,
                                        dest_directory,
                                        common::ArgEndianess::Big,
                                        export_start + 1,
                                        Some(export_end + 1),
                                        &multi_progress,
                                        Some(snd.clone()),
                                    ) {
                                        snd.send(ProgressMessage::Error { msg: e.to_string() })
                                            .unwrap();
                                    }
                                });
                            }
                        });
                        ui.end_row();

                        ui.label("");
                        ui.label(format!(
                            "Note: output frames will be labeled from 1 to {}",
                            data.export_end + 1 - data.export_start
                        ));
                        ui.end_row();

                        if let Some(recv) = &data.background_progress {
                            'multi_messages: loop {
                                match recv.recv_timeout(Duration::from_millis(4)) {
                                    Ok(ProgressMessage::InProgress { num_done, total }) => {
                                        data.background_progress_nums = Some(BgProgress {
                                            done: num_done,
                                            total,
                                        });
                                    }
                                    Ok(ProgressMessage::Done { total: _ }) => {
                                        data.background_progress = None;
                                        data.background_progress_nums = None;
                                        break 'multi_messages;
                                    }
                                    Ok(ProgressMessage::Error { msg }) => {
                                        let err = format!("Error while converting: {msg}");
                                        error!("{err}");
                                        self.error_state = Some(err);
                                        break 'multi_messages;
                                    }
                                    Err(RecvTimeoutError::Timeout) => {
                                        // this is fine.
                                        break 'multi_messages;
                                    }
                                    Err(RecvTimeoutError::Disconnected) => {
                                        error!("background thread disconnected");
                                        // this should only happen if the thread errs out, but that should also
                                        // give us a proper ProgressMessage::Error, so don't try to show this
                                        // in the GUI.
                                        data.background_progress = None;
                                        data.background_progress_nums = None;
                                        break 'multi_messages;
                                    }
                                }
                            }
                            if let Some(prog) = &data.background_progress_nums {
                                ui.label("");
                                ui.add(egui::ProgressBar::new(
                                    prog.done as f32 / prog.total as f32,
                                ));
                                ui.end_row();
                            }
                            // if we expect some progress, we need to redraw:
                            ctx.request_repaint_after(Duration::from_millis(16));
                        }
                    }
                });

            if let Some(data) = &mut self.input_data {
                let view = data.mmap.read_view().unwrap();
                let (nx, ny, _nz) = view.dimensions();

                let texture: &egui::TextureHandle = data.texture.get_or_insert_with(|| {
                    let view = data.mmap.read_view().unwrap();
                    let volume = Volume3D::new(view);
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
