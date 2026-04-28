use std::{
    error::Error,
    path::PathBuf,
    sync::{
        Arc,
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    time::Duration,
};

use clap::Parser;
use eframe::egui::{self, DragValue, RichText, ScrollArea, Slider, Spacing, Style, vec2};
use egui_plot::{Plot, PlotImage, PlotPoint};
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use log::{error, info};

use mrc_to_tiff::{
    common, convert::{self, ProgressMessage}, datasource::{DataSource, load_stack}, render::render_to_rgb
};

#[derive(Parser, Debug)]
struct Args {
    /// Path to the input files (mrc/dm3/dm4). Must be a 3D stack.
    paths: Option<Vec<PathBuf>>,
}

const H: f32 = 15.0;
const V: f32 = 10.0;

#[derive(Default)]
struct ConverterApp {
    dest_directory: Option<PathBuf>,
    quantile: f32,
    multi: MultiProgress,
    state: AppState,
    error_state: Option<String>, // error is another dimension than main app state
}

#[derive(Default)]
enum AppState {
    #[default]
    NoData,
    Convert {
        data: WithInputData,
    },
}

#[derive(Debug)]
struct BgProgress {
    done: usize,
    total: usize,
}

struct WithInputData {
    source_paths: Vec<PathBuf>,
    data_source: Arc<Box<dyn DataSource>>,
    slice_position: usize,
    num_frames: usize,

    export_start: usize,
    export_end: usize,

    texture: Option<egui::TextureHandle>,

    // data for tracking the ongoing export operation (running in a background thread)
    background_progress: Option<Receiver<ProgressMessage>>,
    background_progress_nums: Option<BgProgress>,
}

fn load_data(paths: &[PathBuf]) -> Result<WithInputData, Box<dyn Error + Sync + Send>> {
    let mut paths = paths.to_vec();
    paths.sort_by(|p1, p2| natord::compare(&p1.to_string_lossy(), &p2.to_string_lossy()));
    let data_source = load_stack(&paths)?;
    let num_frames = data_source.dimensions().nz;
    Ok(WithInputData {
        source_paths: paths.to_vec(),
        slice_position: 0,
        num_frames,
        data_source: Arc::new(data_source),
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
            let state = args
                .paths
                .map(|paths| AppState::Convert {
                    data: load_data(&paths).unwrap(),
                })
                .unwrap_or(AppState::NoData);
            let app = ConverterApp {
                state,
                dest_directory: None,
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
            match &mut self.state {
                AppState::NoData => self.render_no_data(ctx),
                AppState::Convert { data: _ } => self.render_with_data(ctx, frame),
            }
        }
    }
}

impl ConverterApp {
    fn pick_files(&self) -> Option<Vec<PathBuf>> {
        rfd::FileDialog::new()
            .add_filter("All supported", &["mrc", "dm3", "dm4"])
            .add_filter("MRC", &["mrc"])
            .add_filter("DM3/DM4", &["dm3", "dm4"])
            .pick_files()
    }

    fn render_no_data(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                let load_btn = egui::Button::new(RichText::new("Open 3D stack...").strong());
                let load_btn = load_btn.fill(egui::Color32::from_rgb(0, 90, 230));

                if ui.add(load_btn).clicked()
                    && let Some(new_paths) = self.pick_files()
                {
                    self.state = match load_data(&new_paths) {
                        Ok(data) => AppState::Convert { data },
                        Err(err) => {
                            self.error_state = Some(format!("Error loading data: {}", err));
                            AppState::NoData
                        }
                    }
                }
            })
        });
    }

    fn render_with_data(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::new(
            egui::panel::TopBottomSide::Bottom,
            "bottom panel view options",
        )
        .frame(egui::containers::Frame::new().inner_margin(vec2(H, V)))
        .show(ctx, |ui| {
            if let AppState::Convert { data } = &mut self.state {
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
            if ui.button("Load 3D stack...").clicked()
                && let Some(new_paths) = self.pick_files()
            {
                self.state = match load_data(&new_paths) {
                    Ok(data) => AppState::Convert { data },
                    Err(err) => {
                        self.error_state = Some(err.to_string());
                        AppState::NoData
                    }
                }
            }

            if let AppState::Convert { data } = &mut self.state {
                ui.label("Input path");
                ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                    for p in &data.source_paths {
                        ui.monospace(p.to_string_lossy());
                    }
                });
            }

            egui::Grid::new("parameter grid")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    if let AppState::Convert { data } = &mut self.state {
                        let dims = data.data_source.dimensions();

                        ui.label("Input size (z, y, x)");
                        ui.monospace(format!("{}x{}x{}", dims.nz, dims.ny, dims.nx));
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

                                let dest_directory = dest_directory.clone();
                                let export_start = data.export_start;
                                let export_end = data.export_end;

                                let bg_data = Arc::clone(&data.data_source);

                                std::thread::spawn(move || {
                                    if let Err(e) = convert::convert(
                                        bg_data,
                                        dest_directory,
                                        common::OutputEndianess::Big,
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

            if let AppState::Convert { data } = &mut self.state {
                let dims = data.data_source.dimensions();
                let nx = dims.nx;
                let ny = dims.ny;

                let texture: &egui::TextureHandle = data.texture.get_or_insert_with(|| {
                    info!("loading slice {}", data.slice_position);
                    let img = render_to_rgb(
                        &data.data_source.get_slice_f32(data.slice_position),
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
                    let aspect_ratio = nx as f32 / ny as f32;
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
