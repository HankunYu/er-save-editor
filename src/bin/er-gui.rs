// er-gui: egui front-end for the er-qs Elden Ring save toolkit.
// Build with: cargo build --release --features gui

use eframe::egui;
use er_qs::*;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([920.0, 640.0])
            .with_title("ER 存档工坊"),
        ..Default::default()
    };
    eframe::run_native(
        "ER 存档工坊",
        options,
        Box::new(|cc| {
            setup_cjk_fonts(&cc.egui_ctx);
            Ok(Box::new(App::new()) as Box<dyn eframe::App>)
        }),
    )
}

enum Action {
    Refresh,
    Save(String),
    Load(String),
    Delete(String),
}

struct App {
    save: Option<SaveFile>,
    chars: Vec<Character>,
    steam_id: Option<u64>,
    snapshots: Vec<SnapshotInfo>,
    new_name: String,
    status: String,
}

impl App {
    fn new() -> Self {
        let mut app = App {
            save: None,
            chars: Vec::new(),
            steam_id: None,
            snapshots: Vec::new(),
            new_name: String::new(),
            status: String::new(),
        };
        app.refresh();
        app
    }

    fn refresh(&mut self) {
        match SaveFile::locate_and_load(None) {
            Ok(sf) => {
                self.chars = sf.characters();
                self.steam_id = sf.steam_id();
                self.save = Some(sf);
                self.status = "已刷新".into();
            }
            Err(e) => {
                self.save = None;
                self.chars.clear();
                self.steam_id = None;
                self.status = format!("找不到存档: {e}");
            }
        }
        self.snapshots = list_snapshots().unwrap_or_default();
    }

    fn save_path(&self) -> Option<PathBuf> {
        self.save.as_ref().map(|s| s.path.clone())
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::Refresh => self.refresh(),
            Action::Save(slot) => {
                if let Some(path) = self.save_path() {
                    // GUI uses no-wait so the click feels instant; the user
                    // snapshots at a grace where the file is already settled.
                    match save_snapshot(&path, &slot, false) {
                        Ok(n) => {
                            self.status = format!("✓ 已存快照 '{}' ({})", display_slot(&slot), human_size(n))
                        }
                        Err(e) => self.status = format!("存档失败: {e}"),
                    }
                    self.snapshots = list_snapshots().unwrap_or_default();
                } else {
                    self.status = "没有定位到存档".into();
                }
            }
            Action::Load(slot) => {
                if let Some(path) = self.save_path() {
                    match load_snapshot(&path, &slot) {
                        Ok(_) => {
                            self.status =
                                format!("✓ 已读档 '{}' — 记得在标题画面选「继续游戏」", display_slot(&slot))
                        }
                        Err(e) => self.status = format!("读档失败: {e}"),
                    }
                } else {
                    self.status = "没有定位到存档".into();
                }
            }
            Action::Delete(slot) => {
                match delete_snapshot(&slot) {
                    Ok(_) => self.status = format!("已删除快照 '{}'", display_slot(&slot)),
                    Err(e) => self.status = format!("删除失败: {e}"),
                }
                self.snapshots = list_snapshots().unwrap_or_default();
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Snapshot state for the UI closures; collect one action, apply after.
        let chars = self.chars.clone();
        let snapshots = self.snapshots.clone();
        let steam = self.steam_id;
        let path = self.save.as_ref().map(|s| s.path.display().to_string());
        let status = self.status.clone();
        let mut new_name = self.new_name.clone();
        let mut action: Option<Action> = None;

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🗡 ER 存档工坊");
                if ui.button("🔄 刷新").clicked() {
                    action = Some(Action::Refresh);
                }
            });
            match &path {
                Some(p) => ui.label(format!("存档: {p}")),
                None => ui.colored_label(egui::Color32::LIGHT_RED, "未定位到存档"),
            };
            if let Some(sid) = steam {
                ui.label(format!("SteamID: {sid}"));
            }
            if !status.is_empty() {
                ui.colored_label(egui::Color32::from_rgb(120, 200, 120), &status);
            }
        });

        egui::SidePanel::right("snapshots")
            .default_width(340.0)
            .show(ctx, |ui| {
                ui.heading("快照管理");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button("⚡ 快速存档").clicked() {
                        action = Some(Action::Save(QUICK_SLOT.to_string()));
                    }
                    if ui.button("📥 快速读档").clicked() {
                        action = Some(Action::Load(QUICK_SLOT.to_string()));
                    }
                });
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut new_name);
                    if ui.button("➕ 存为").clicked() {
                        let n = new_name.trim().to_string();
                        if !n.is_empty() {
                            action = Some(Action::Save(n));
                            new_name.clear();
                        }
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if snapshots.is_empty() {
                        ui.weak("(还没有快照)");
                    }
                    for s in &snapshots {
                        ui.horizontal(|ui| {
                            let age = s
                                .modified
                                .elapsed()
                                .map(|d| fmt_relative(d.as_secs()))
                                .unwrap_or_else(|_| "now".into());
                            ui.label(format!(
                                "{}  ·  {}  ·  {age}",
                                display_slot(&s.name),
                                human_size(s.size)
                            ));
                            if ui.small_button("读").clicked() {
                                action = Some(Action::Load(s.name.clone()));
                            }
                            if ui.small_button("删").clicked() {
                                action = Some(Action::Delete(s.name.clone()));
                            }
                        });
                    }
                });
                ui.add_space(8.0);
                ui.weak("读档后请在游戏标题画面选「继续游戏」生效");
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("角色");
            ui.add_space(4.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                if chars.is_empty() {
                    ui.weak("(没有角色 — 建个角色存盘后会显示)");
                }
                for c in &chars {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.strong(format!("[槽{}] {}", c.slot, c.name));
                            ui.label(format!("Lv{}", c.level));
                            let (h, m) = c.playtime_hm();
                            ui.weak(format!("· {h}h{m}m"));
                        });
                        ui.horizontal_wrapped(|ui| {
                            for (n, v) in STAT_NAMES.iter().zip(c.stats.iter()) {
                                ui.label(format!("{n} {v}"));
                            }
                        });
                    });
                    ui.add_space(4.0);
                }
            });
        });

        self.new_name = new_name;
        if let Some(a) = action {
            self.apply(a);
        }
    }
}

// Load a system CJK font so Chinese renders instead of tofu boxes.
fn setup_cjk_fonts(ctx: &egui::Context) {
    let candidates = [
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJKsc-Regular.otf",
        "/usr/share/fonts/noto-cjk/NotoSerifCJK-Regular.ttc",
        "/usr/share/fonts/adobe-source-han-sans-otc-fonts/SourceHanSans.ttc",
        "/usr/share/fonts/wenquanyi/wqy-microhei/wqy-microhei.ttc",
        "/usr/share/fonts/TTF/wqy-microhei.ttc",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("cjk".to_owned(), egui::FontData::from_owned(bytes));
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "cjk".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("cjk".to_owned());
            ctx.set_fonts(fonts);
            return;
        }
    }
}
