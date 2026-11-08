use std::path::PathBuf;

use eframe::egui;
use rfi_frequency::{Info, decode, inspect};

fn basis_name(basis: u16) -> &'static str {
    match basis {
        1 => "DCT8",
        3 => "DCT16",
        _ => "unknown basis",
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    let mut initial = None;
    let mut check = false;
    let mut shot = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--file" | "-f" => {
                i += 1;
                initial = args.get(i).map(PathBuf::from);
            }
            "--check" => {
                i += 1;
                check = true;
                initial = args.get(i).map(PathBuf::from);
            }
            "--screenshot" => {
                i += 1;
                shot = args.get(i).map(PathBuf::from);
            }
            other if !other.starts_with('-') => initial = Some(PathBuf::from(other)),
            _ => {}
        }
        i += 1;
    }
    if check {
        let Some(path) = initial else {
            eprintln!("usage: rfi-viewer --check <file.rfi>");
            return Ok(());
        };
        match Loaded::from_path(&path) {
            Ok(loaded) => println!(
                "{}: {}x{}, {} bytes, {} qstep {:.2}",
                path.display(),
                loaded.info.width,
                loaded.info.height,
                loaded.bytes.len(),
                basis_name(loaded.info.basis),
                loaded.info.step
            ),
            Err(error) => {
                eprintln!("error: {error}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 760.0])
            .with_title("RFI Viewer"),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "RFI Viewer",
        options,
        Box::new(move |cc| Ok(Box::new(Viewer::new(cc, initial.clone(), shot.clone())))),
    )
}

struct Loaded {
    bytes: Vec<u8>,
    image: image::RgbaImage,
    info: Info,
    texture: Option<egui::TextureHandle>,
}

impl Loaded {
    fn from_path(path: &PathBuf) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read: {e}"))?;
        let info = inspect(&bytes).map_err(|e| format!("inspect: {e}"))?;
        let image = decode(&bytes).map_err(|e| format!("decode: {e}"))?;
        Ok(Self {
            bytes,
            image,
            info,
            texture: None,
        })
    }

    fn upload(&mut self, ctx: &egui::Context) {
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [self.image.width() as usize, self.image.height() as usize],
            self.image.as_raw(),
        );
        self.texture = Some(ctx.load_texture("rfi", color, egui::TextureOptions::LINEAR));
    }
}

struct Viewer {
    loaded: Option<Loaded>,
    path: Option<PathBuf>,
    error: Option<String>,
    zoom: f32,
    fit: bool,
    checker: bool,
    tiles: bool,
    shot: Option<PathBuf>,
    shot_done: bool,
}

impl Viewer {
    fn new(cc: &eframe::CreationContext<'_>, initial: Option<PathBuf>, shot: Option<PathBuf>) -> Self {
        let mut app = Self {
            loaded: None,
            path: None,
            error: None,
            zoom: 1.0,
            fit: true,
            checker: true,
            tiles: false,
            shot,
            shot_done: false,
        };
        if let Some(path) = initial {
            app.open(&cc.egui_ctx, path);
        }
        app
    }

    fn open(&mut self, ctx: &egui::Context, path: PathBuf) {
        match Loaded::from_path(&path) {
            Ok(mut loaded) => {
                loaded.upload(ctx);
                self.loaded = Some(loaded);
                self.path = Some(path);
                self.error = None;
                self.fit = true;
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn dialog(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new().add_filter("RFI", &["rfi"]).pick_file() {
            self.open(ctx, path);
        }
    }

    fn view(&mut self, ui: &mut egui::Ui) {
        let Some(loaded) = self.loaded.as_mut() else {
            ui.centered_and_justified(|ui| ui.label("Open an .rfi file."));
            return;
        };
        let size = egui::vec2(loaded.image.width() as f32, loaded.image.height() as f32);
        let available = ui.available_size();
        let fit = (available.x / size.x).min(available.y / size.y).max(0.01);
        let scale = if self.fit { fit } else { self.zoom };
        let display = size * scale;
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let (rect, _) = ui.allocate_exact_size(display, egui::Sense::click_and_drag());
                let painter = ui.painter_at(rect);
                if self.checker {
                    let cell = 16.0;
                    let mut y = rect.top();
                    let mut row = 0;
                    while y < rect.bottom() {
                        let mut x = rect.left();
                        let mut col = 0;
                        while x < rect.right() {
                            let shade = if (row + col) % 2 == 0 { 52 } else { 38 };
                            painter.rect_filled(
                                egui::Rect::from_min_max(
                                    egui::pos2(x, y),
                                    egui::pos2((x + cell).min(rect.right()), (y + cell).min(rect.bottom())),
                                ),
                                0.0,
                                egui::Color32::from_gray(shade),
                            );
                            x += cell;
                            col += 1;
                        }
                        y += cell;
                        row += 1;
                    }
                }
                if let Some(texture) = loaded.texture.as_ref() {
                    painter.image(
                        texture.id(),
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
                if self.tiles {
                    let tile = loaded.info.tile_size as f32 * scale;
                    let mut x = rect.left();
                    while x <= rect.right() {
                        painter.line_segment(
                            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                            (1.0, egui::Color32::from_rgba_unmultiplied(255, 200, 0, 120)),
                        );
                        x += tile;
                    }
                    let mut y = rect.top();
                    while y <= rect.bottom() {
                        painter.line_segment(
                            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                            (1.0, egui::Color32::from_rgba_unmultiplied(255, 200, 0, 120)),
                        );
                        y += tile;
                    }
                }
            });
        ui.label(format!(
            "{}x{}  {:.0}%",
            loaded.info.width,
            loaded.info.height,
            scale * 100.0
        ));
    }

    fn info(&self, ui: &mut egui::Ui) {
        let Some(loaded) = self.loaded.as_ref() else {
            ui.label("No file loaded.");
            return;
        };
        let info = loaded.info;
        egui::Grid::new("info")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Size");
                ui.label(format!("{} x {}", info.width, info.height));
                ui.end_row();
                ui.label("File");
                ui.label(format!("{} bytes", loaded.bytes.len()));
                ui.end_row();
                ui.label("Qstep");
                ui.label(format!("{:.2}", info.step));
                ui.end_row();
                ui.label("Basis");
                ui.label(info.basis.to_string());
                ui.end_row();
                ui.label("Sections");
                ui.label(info.section_count.to_string());
                ui.end_row();
            });
        ui.separator();
        ui.label("New RFI: DCT8/DCT16 coefficient sections; no RGBA residual section.");
    }
}

impl eframe::App for Viewer {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(path) = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .first()
                .and_then(|file| file.path().to_path_buf().into())
        }) {
            self.open(&ctx, path);
        }
        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open...").clicked() {
                        ui.close();
                        self.dialog(&ctx);
                    }
                    if ui.button("Reload").clicked() {
                        ui.close();
                        if let Some(path) = self.path.clone() {
                            self.open(&ctx, path);
                        }
                    }
                    if ui.button("Close").clicked() {
                        ui.close();
                        self.loaded = None;
                    }
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.fit, "Fit");
                    ui.checkbox(&mut self.checker, "Checkerboard");
                    ui.checkbox(&mut self.tiles, "Tile grid");
                    ui.add(egui::Slider::new(&mut self.zoom, 0.05..=8.0).text("Zoom"));
                });
                ui.separator();
                if let Some(path) = &self.path {
                    ui.label(path.display().to_string());
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("eframe / wgpu");
                });
            });
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            if let Some(error) = &self.error {
                ui.colored_label(egui::Color32::RED, error);
            } else if let Some(loaded) = &self.loaded {
                ui.label(format!(
                    "{} bytes  |  DCT8  |  qstep {:.2}",
                    loaded.bytes.len(),
                    loaded.info.step
                ));
            } else {
                ui.label("No file loaded");
            }
        });
        egui::Panel::left("info")
            .resizable(true)
            .default_size(280.0)
            .show(ui, |ui| {
                ui.heading("RFI");
                ui.separator();
                self.info(ui);
            });
        egui::CentralPanel::default().show(ui, |ui| self.view(ui));
        if let Some(path) = self.shot.clone()
            && !self.shot_done
            && self.loaded.is_some()
        {
            self.shot_done = true;
            ctx.request_screenshot(move |image| {
                let _ = image::save_buffer(
                    &path,
                    image.as_raw(),
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                );
            });
            ctx.request_repaint();
        }
    }
}
