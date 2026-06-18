// er: ratatui terminal UI for the Elden Ring save toolkit (main front-end).
// Build with: cargo build --release --features tui
//
// Uses the terminal's own font (no CJK font loading needed). The real in-game
// quick save/load goes through the CLI bound to Hyprland hotkeys; this is the
// management / inspection / edit front-end.

use er_qs::*;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};
use ratatui::{DefaultTerminal, Frame};
use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = App::load();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}

#[derive(PartialEq, Clone, Copy)]
enum Focus {
    Snapshots,
    Chars,
}

// Normal split view vs the item editor for the selected character.
#[derive(PartialEq, Clone, Copy)]
enum Mode {
    Normal,
    Items,
    AddItem,
}

// What the footer text-input line is collecting, if anything.
#[derive(PartialEq, Clone, Copy)]
enum Input {
    None,
    NewSnapshot,
    Rename,
    Runes,
    ItemQty,
    AddQty,
}

struct App {
    save_path: Option<PathBuf>,
    steam_id: Option<u64>,
    chars: Vec<Character>,
    snapshots: Vec<SnapshotInfo>,
    focus: Focus,
    char_sel: usize,
    snap_sel: usize,
    mode: Mode,
    items: Vec<(&'static str, u32, u32)>,
    item_sel: usize,
    // AddItem mode: live-filtered list of all addable items.
    add_query: String,
    add_results: Vec<(u32, &'static str)>,
    add_sel: usize,
    input: Input,
    input_buffer: String,
    status: String,
    should_quit: bool,
}

impl App {
    fn load() -> Self {
        let mut app = App {
            save_path: None,
            steam_id: None,
            chars: Vec::new(),
            snapshots: Vec::new(),
            focus: Focus::Snapshots,
            char_sel: 0,
            snap_sel: 0,
            mode: Mode::Normal,
            items: Vec::new(),
            item_sel: 0,
            add_query: String::new(),
            add_results: Vec::new(),
            add_sel: 0,
            input: Input::None,
            input_buffer: String::new(),
            status: "就绪".into(),
            should_quit: false,
        };
        app.refresh();
        app
    }

    fn refresh(&mut self) {
        match SaveFile::locate_and_load(None) {
            Ok(sf) => {
                self.chars = sf.characters();
                self.steam_id = sf.steam_id();
                self.save_path = Some(sf.path);
            }
            Err(e) => {
                self.status = format!("找不到存档: {e}");
                self.save_path = None;
                self.chars.clear();
                self.steam_id = None;
            }
        }
        self.snapshots = list_snapshots().unwrap_or_default();
        if self.char_sel >= self.chars.len() {
            self.char_sel = self.chars.len().saturating_sub(1);
        }
        if self.snap_sel >= self.snapshots.len() {
            self.snap_sel = self.snapshots.len().saturating_sub(1);
        }
    }

    fn sel_slot(&self) -> Option<usize> {
        self.chars.get(self.char_sel).map(|c| c.slot)
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        while !self.should_quit {
            terminal.draw(|f| ui(f, self))?;
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    self.handle_key(key.code);
                }
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, code: KeyCode) {
        if self.input != Input::None {
            self.handle_input_key(code);
            return;
        }
        match self.mode {
            Mode::Items => return self.handle_items_key(code),
            Mode::AddItem => return self.handle_additem_key(code),
            Mode::Normal => {}
        }
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('r') => self.refresh(),
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Snapshots => Focus::Chars,
                    Focus::Chars => Focus::Snapshots,
                };
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_sel(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_sel(-1),
            KeyCode::Char('s') => self.save_quick(),
            KeyCode::Char('l') => self.load_quick(),
            KeyCode::Char('n') => {
                self.input = Input::NewSnapshot;
                self.input_buffer.clear();
            }
            _ => match self.focus {
                Focus::Snapshots => match code {
                    KeyCode::Enter => self.load_selected_snapshot(),
                    KeyCode::Char('d') => self.delete_selected_snapshot(),
                    _ => {}
                },
                Focus::Chars => match code {
                    KeyCode::Char('g') if self.sel_slot().is_some() => {
                        self.input = Input::Runes;
                        self.input_buffer.clear();
                    }
                    KeyCode::Char('R') if self.sel_slot().is_some() => {
                        self.input = Input::Rename;
                        self.input_buffer.clear();
                    }
                    KeyCode::Char('i') if self.sel_slot().is_some() => self.enter_items(),
                    _ => {}
                },
            },
        }
    }

    fn handle_items_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc | KeyCode::Char('i') | KeyCode::Char('q') => self.mode = Mode::Normal,
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.items.is_empty() {
                    self.item_sel = (self.item_sel + 1).min(self.items.len() - 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.item_sel = self.item_sel.saturating_sub(1);
            }
            KeyCode::Enter | KeyCode::Char('e') => {
                if !self.items.is_empty() {
                    self.input = Input::ItemQty;
                    self.input_buffer.clear();
                }
            }
            KeyCode::Char('a') => self.enter_additem(),
            _ => {}
        }
    }

    fn enter_additem(&mut self) {
        self.add_query.clear();
        self.add_results = item_search("");
        self.add_sel = 0;
        self.mode = Mode::AddItem;
        self.status = "打字过滤 · ↑↓ 选 · Enter 加 · Esc 返回".into();
    }

    fn handle_additem_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => self.mode = Mode::Items,
            KeyCode::Up => self.add_sel = self.add_sel.saturating_sub(1),
            KeyCode::Down => {
                if !self.add_results.is_empty() {
                    self.add_sel = (self.add_sel + 1).min(self.add_results.len() - 1);
                }
            }
            KeyCode::Enter => {
                if !self.add_results.is_empty() {
                    self.input = Input::AddQty;
                    self.input_buffer.clear();
                }
            }
            KeyCode::Backspace => {
                self.add_query.pop();
                self.refilter_add();
            }
            KeyCode::Char(c) => {
                self.add_query.push(c);
                self.refilter_add();
            }
            _ => {}
        }
    }

    fn refilter_add(&mut self) {
        self.add_results = item_search(&self.add_query);
        if self.add_sel >= self.add_results.len() {
            self.add_sel = self.add_results.len().saturating_sub(1);
        }
    }

    fn enter_items(&mut self) {
        let (Some(p), Some(slot)) = (self.save_path.clone(), self.sel_slot()) else {
            return;
        };
        match SaveFile::load(p) {
            Ok(sf) => {
                self.items = sf.owned_items(slot);
                self.item_sel = 0;
                self.mode = Mode::Items;
                self.status = if self.items.is_empty() {
                    "该角色没有可识别的常用物品".into()
                } else {
                    format!("{} 种物品 — [↑↓]选 [Enter]改数量 [Esc]返回", self.items.len())
                };
            }
            Err(e) => self.status = format!("读取失败: {e}"),
        }
    }

    fn handle_input_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Enter => {
                let buf = self.input_buffer.trim().to_string();
                let kind = self.input;
                self.input = Input::None;
                self.input_buffer.clear();
                self.commit_input(kind, buf);
            }
            KeyCode::Esc => {
                self.input = Input::None;
                self.input_buffer.clear();
                self.status = "已取消".into();
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Char(c) => self.input_buffer.push(c),
            _ => {}
        }
    }

    fn commit_input(&mut self, kind: Input, buf: String) {
        match kind {
            Input::NewSnapshot => {
                if buf.is_empty() {
                    self.status = "已取消(空名字)".into();
                } else {
                    self.save_named(buf);
                }
            }
            Input::Rename => self.do_rename(buf),
            Input::Runes => self.do_runes(buf),
            Input::ItemQty => self.do_item_qty(buf),
            Input::AddQty => self.do_add_qty(buf),
            Input::None => {}
        }
    }

    fn move_sel(&mut self, delta: i32) {
        match self.focus {
            Focus::Snapshots => {
                if self.snapshots.is_empty() {
                    return;
                }
                let n = self.snapshots.len() as i32;
                self.snap_sel = (self.snap_sel as i32 + delta).clamp(0, n - 1) as usize;
            }
            Focus::Chars => {
                if self.chars.is_empty() {
                    return;
                }
                let n = self.chars.len() as i32;
                self.char_sel = (self.char_sel as i32 + delta).clamp(0, n - 1) as usize;
            }
        }
    }

    // --- snapshot ops ---
    fn save_quick(&mut self) {
        if let Some(p) = self.save_path.clone() {
            match save_snapshot(&p, QUICK_SLOT, false) {
                Ok(n) => self.status = format!("✓ 已存 [quick] ({})", human_size(n)),
                Err(e) => self.status = format!("存档失败: {e}"),
            }
            self.refresh();
        } else {
            self.status = "没有定位到存档".into();
        }
    }

    fn save_named(&mut self, name: String) {
        if let Some(p) = self.save_path.clone() {
            match save_snapshot(&p, &name, false) {
                Ok(n) => self.status = format!("✓ 已存快照 '{name}' ({})", human_size(n)),
                Err(e) => self.status = format!("存档失败: {e}"),
            }
            self.refresh();
        }
    }

    fn load_quick(&mut self) {
        self.load_slot(QUICK_SLOT.to_string());
    }

    fn load_selected_snapshot(&mut self) {
        if let Some(s) = self.snapshots.get(self.snap_sel) {
            self.load_slot(s.name.clone());
        }
    }

    fn load_slot(&mut self, slot: String) {
        if let Some(p) = self.save_path.clone() {
            match load_snapshot(&p, &slot) {
                Ok(_) => {
                    self.status =
                        format!("✓ 已读 '{}' — 在游戏标题画面选「继续游戏」生效", display_slot(&slot))
                }
                Err(e) => self.status = format!("读档失败: {e}"),
            }
        }
    }

    fn delete_selected_snapshot(&mut self) {
        if let Some(s) = self.snapshots.get(self.snap_sel).cloned() {
            match delete_snapshot(&s.name) {
                Ok(_) => self.status = format!("已删除快照 '{}'", display_slot(&s.name)),
                Err(e) => self.status = format!("删除失败: {e}"),
            }
            self.refresh();
        }
    }

    // --- character edits (write to the live save file, with auto-backup) ---
    fn do_rename(&mut self, name: String) {
        let Some(slot) = self.sel_slot() else {
            self.status = "没有选中角色".into();
            return;
        };
        if name.is_empty() {
            self.status = "已取消(空名字)".into();
            return;
        }
        match self.edit_save(|sf| sf.set_name(slot, &name)) {
            Ok(_) => self.status = format!("✓ 角色已改名为 '{name}'(已写回存档)"),
            Err(e) => self.status = format!("改名失败: {e}"),
        }
    }

    fn do_runes(&mut self, buf: String) {
        let Some(slot) = self.sel_slot() else {
            self.status = "没有选中角色".into();
            return;
        };
        let new = match buf.parse::<u32>() {
            Ok(v) => v,
            Err(_) => {
                self.status = "请输入一个数字(新卢恩数)".into();
                return;
            }
        };
        match self.edit_save(|sf| sf.set_runes(slot, new)) {
            Ok(_) => self.status = format!("✓ 卢恩已改为 {new}(已写回存档)"),
            Err(e) => self.status = format!("改卢恩失败: {e}"),
        }
    }

    fn do_item_qty(&mut self, buf: String) {
        let Some(slot) = self.sel_slot() else {
            return;
        };
        let Some(&(name, base_id, _)) = self.items.get(self.item_sel) else {
            return;
        };
        let qty = match buf.parse::<u32>() {
            Ok(v) => v,
            Err(_) => {
                self.status = "请输入一个数字".into();
                return;
            }
        };
        match self.edit_save(|sf| sf.set_item_quantity(slot, base_id, qty)) {
            Ok(_) => {
                self.status = format!("✓ {name} 数量改为 {qty}(已写回)");
                if let (Some(p), Some(s)) = (self.save_path.clone(), self.sel_slot()) {
                    if let Ok(sf) = SaveFile::load(p) {
                        self.items = sf.owned_items(s);
                    }
                }
            }
            Err(e) => self.status = format!("改物品失败: {e}"),
        }
    }

    fn do_add_qty(&mut self, buf: String) {
        let Some(slot) = self.sel_slot() else {
            self.status = "没有选中角色".into();
            return;
        };
        let qty = match buf.trim().parse::<u32>() {
            Ok(n) => n,
            Err(_) => {
                self.status = "请输入一个数字".into();
                return;
            }
        };
        let Some(&(base_id, name)) = self.add_results.get(self.add_sel) else {
            return;
        };
        match self.edit_save(|sf| sf.give_item(slot, base_id, qty)) {
            Ok(_) => {
                self.status = format!("✓ 已加 {name} ×{qty}(已写回)");
                if let (Some(p), Some(s)) = (self.save_path.clone(), self.sel_slot()) {
                    if let Ok(sf) = SaveFile::load(p) {
                        self.items = sf.owned_items(s);
                    }
                }
                self.mode = Mode::Items;
            }
            Err(e) => self.status = format!("加物品失败: {e}"),
        }
    }

    // Load the save fresh, apply an edit, write it back (with backup). Refreshes.
    fn edit_save<F>(&mut self, f: F) -> Result<(), String>
    where
        F: FnOnce(&mut SaveFile) -> Result<(), String>,
    {
        let p = self.save_path.clone().ok_or("没有定位到存档")?;
        let mut sf = SaveFile::load(p)?;
        f(&mut sf)?;
        sf.write_back()?;
        self.refresh();
        Ok(())
    }
}

fn ui(frame: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(5),
        Constraint::Length(4),
    ])
    .split(frame.area());
    let body = Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(chunks[1]);

    // --- header ---
    let path = app
        .save_path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "未定位到存档".into());
    let steam = app
        .steam_id
        .map(|s| s.to_string())
        .unwrap_or_else(|| "-".into());
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!("存档: {path}")),
            Line::from(format!("SteamID: {steam}")),
        ])
        .block(Block::bordered().title(" 🗡 ER 存档工坊 ")),
        chunks[0],
    );

    // --- left panel: characters / item editor / add-item picker ---
    if app.mode == Mode::AddItem {
        let owner = app
            .chars
            .get(app.char_sel)
            .map(|c| c.name.as_str())
            .unwrap_or("?");
        let rows: Vec<ListItem> = if app.add_results.is_empty() {
            vec![ListItem::new(Line::from("(无匹配)").dim())]
        } else {
            app.add_results
                .iter()
                .map(|&(id, name)| ListItem::new(format!("{name}  (id {id})")))
                .collect()
        };
        let mut state = ListState::default();
        if !app.add_results.is_empty() {
            state.select(Some(app.add_sel));
        }
        let title = format!(
            " 加物品 → {owner}  搜索: {}  ({}/{}) ",
            app.add_query,
            app.add_sel + 1,
            app.add_results.len()
        );
        frame.render_stateful_widget(
            List::new(rows)
                .block(Block::bordered().title(title))
                .highlight_style(Style::new().reversed())
                .highlight_symbol("> "),
            body[0],
            &mut state,
        );
    } else if app.mode == Mode::Items {
        let owner = app
            .chars
            .get(app.char_sel)
            .map(|c| c.name.as_str())
            .unwrap_or("?");
        let item_rows: Vec<ListItem> = if app.items.is_empty() {
            vec![ListItem::new(Line::from("(没有可识别的常用物品)").dim())]
        } else {
            app.items
                .iter()
                .map(|(name, _id, qty)| ListItem::new(format!("{name}  ×{qty}")))
                .collect()
        };
        let mut item_state = ListState::default();
        if !app.items.is_empty() {
            item_state.select(Some(app.item_sel));
        }
        frame.render_stateful_widget(
            List::new(item_rows)
                .block(Block::bordered().title(format!(" 物品 — {owner} ")))
                .highlight_style(Style::new().reversed())
                .highlight_symbol("> "),
            body[0],
            &mut item_state,
        );
    } else {
        let char_items: Vec<ListItem> = if app.chars.is_empty() {
            vec![ListItem::new(Line::from("(没有角色)").dim())]
        } else {
            app.chars
                .iter()
                .map(|c| {
                    let (h, m) = c.playtime_hm();
                    let head = format!(
                        "[槽{}] {}  Lv{}  {}符文  {}h{}m",
                        c.slot, c.name, c.level, c.runes, h, m
                    );
                    let parts: Vec<String> = STAT_NAMES
                        .iter()
                        .zip(c.stats.iter())
                        .map(|(n, v)| {
                            let pad = if n.chars().count() >= 3 { "" } else { "  " };
                            format!("{n}{pad}{v:>2}")
                        })
                        .collect();
                    ListItem::new(vec![
                        Line::from(head),
                        Line::from(format!("  {}", parts[0..4].join("  "))).dim(),
                        Line::from(format!("  {}", parts[4..8].join("  "))).dim(),
                    ])
                })
                .collect()
        };
        let char_title = if app.focus == Focus::Chars {
            " 角色 ◀ "
        } else {
            " 角色 "
        };
        let mut char_state = ListState::default();
        if app.focus == Focus::Chars && !app.chars.is_empty() {
            char_state.select(Some(app.char_sel));
        }
        frame.render_stateful_widget(
            List::new(char_items)
                .block(Block::bordered().title(char_title))
                .highlight_style(Style::new().reversed()),
            body[0],
            &mut char_state,
        );
    }

    // --- right: snapshots ---
    let snap_items: Vec<ListItem> = app
        .snapshots
        .iter()
        .map(|s| {
            let age = s
                .modified
                .elapsed()
                .map(|d| fmt_relative(d.as_secs()))
                .unwrap_or_else(|_| "now".into());
            ListItem::new(format!(
                "{}  ·  {}  ·  {age}",
                display_slot(&s.name),
                human_size(s.size)
            ))
        })
        .collect();
    let snap_title = if app.focus == Focus::Snapshots {
        " 快照 ◀ "
    } else {
        " 快照 "
    };
    let mut snap_state = ListState::default();
    if app.focus == Focus::Snapshots && !app.snapshots.is_empty() {
        snap_state.select(Some(app.snap_sel));
    }
    frame.render_stateful_widget(
        List::new(snap_items)
            .block(Block::bordered().title(snap_title))
            .highlight_style(Style::new().reversed())
            .highlight_symbol("> "),
        body[1],
        &mut snap_state,
    );

    // --- footer: status + context help, or an input prompt ---
    let footer_lines = match app.input {
        Input::None => {
            let help = if app.mode == Mode::AddItem {
                "打字过滤  [↑↓]选  [Enter]加  [Backspace]删字  [Esc]返回"
            } else if app.mode == Mode::Items {
                "[↑↓]选物品  [Enter]改数量  [a]加物品  [Esc/i]返回  [q]退出"
            } else {
                match app.focus {
                    Focus::Snapshots => {
                        "[Tab]→角色 [↑↓]选 [Enter]读 [d]删快照 │ [s]存 [l]读 [n]命名存 [r]刷新 [q]退"
                    }
                    Focus::Chars => {
                        "[Tab]→快照 [↑↓]选角色 [g]改卢恩 [R]改名 [i]物品 │ [s]存 [l]读 [n]命名存 [q]退"
                    }
                }
            };
            vec![
                Line::from(app.status.clone()).green(),
                Line::from(help).dim(),
            ]
        }
        Input::ItemQty => {
            let info = app
                .items
                .get(app.item_sel)
                .map(|(n, _, q)| format!("{n} (当前×{q})"))
                .unwrap_or_default();
            vec![
                Line::from(format!("改 {info} 数量为: {}_", app.input_buffer)).yellow(),
                Line::from("[Enter] 确认    [Esc] 取消").dim(),
            ]
        }
        Input::AddQty => {
            let name = app.add_results.get(app.add_sel).map(|&(_, n)| n).unwrap_or("");
            vec![
                Line::from(format!("加 {name} 数量: {}_", app.input_buffer)).yellow(),
                Line::from("[Enter] 确认    [Esc] 取消").dim(),
            ]
        }
        Input::NewSnapshot => vec![
            Line::from(format!("新快照名: {}_", app.input_buffer)).yellow(),
            Line::from("[Enter] 确认    [Esc] 取消").dim(),
        ],
        Input::Rename => vec![
            Line::from(format!("改角色名为: {}_", app.input_buffer)).yellow(),
            Line::from("[Enter] 确认    [Esc] 取消").dim(),
        ],
        Input::Runes => {
            let cur = app.chars.get(app.char_sel).map(|c| c.runes).unwrap_or(0);
            vec![
                Line::from(format!("改卢恩 (当前 {cur},输入新值): {}_", app.input_buffer)).yellow(),
                Line::from("[Enter] 确认    [Esc] 取消").dim(),
            ]
        }
    };
    frame.render_widget(
        Paragraph::new(footer_lines).block(Block::bordered()),
        chunks[2],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::time::SystemTime;

    fn sample_app() -> App {
        App {
            save_path: Some(PathBuf::from("…/EldenRing/76561…/ER0000.sl2")),
            steam_id: Some(76561198139999795),
            chars: vec![
                Character {
                    slot: 0,
                    name: "Hankun".into(),
                    level: 1,
                    stats: [10; 8],
                    runes: 5,
                    playtime_secs: 1800,
                },
                Character {
                    slot: 1,
                    name: "Lilith".into(),
                    level: 43,
                    stats: [24, 11, 13, 12, 17, 9, 8, 28],
                    runes: 1300,
                    playtime_secs: 36060,
                },
            ],
            snapshots: vec![SnapshotInfo {
                name: "_quick".into(),
                modified: SystemTime::now(),
                size: 28_967_888,
            }],
            focus: Focus::Chars,
            char_sel: 1,
            snap_sel: 0,
            mode: Mode::Normal,
            items: Vec::new(),
            item_sel: 0,
            add_query: String::new(),
            add_results: Vec::new(),
            add_sel: 0,
            input: Input::None,
            input_buffer: String::new(),
            status: "就绪".into(),
            should_quit: false,
        }
    }

    #[test]
    fn renders_layout() {
        let backend = TestBackend::new(96, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = sample_app();
        terminal.draw(|f| ui(f, &app)).unwrap();
        println!("{}", terminal.backend());
        let rendered = format!("{}", terminal.backend());
        assert!(rendered.contains("Lilith"), "character missing");
        assert!(rendered.contains("角色"), "char panel missing");
        assert!(rendered.contains("快照"), "snapshot panel missing");
        assert!(rendered.contains("改卢恩"), "edit help missing when focus=Chars");
    }

    #[test]
    fn renders_items_mode() {
        let backend = TestBackend::new(96, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = sample_app();
        app.mode = Mode::Items;
        app.items = vec![
            ("Smithing Stone [2]", 10101, 19),
            ("Lord's Rune", 2919, 5),
            ("Larval Tear", 8185, 3),
        ];
        app.item_sel = 0;
        terminal.draw(|f| ui(f, &app)).unwrap();
        println!("{}", terminal.backend());
        let r = format!("{}", terminal.backend());
        assert!(r.contains("Smithing Stone"), "item missing");
        assert!(r.contains("物品"), "item panel title missing");
    }

    #[test]
    fn renders_additem_mode() {
        let backend = TestBackend::new(96, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = sample_app();
        app.mode = Mode::AddItem;
        app.add_query = "锻造".to_string();
        app.add_results = item_search("锻造");
        app.add_sel = 0;
        terminal.draw(|f| ui(f, &app)).unwrap();
        println!("{}", terminal.backend());
        let r = format!("{}", terminal.backend());
        assert!(r.contains("加物品"), "add-item title missing");
        assert!(r.contains("锻造石"), "filtered item missing");
    }
}
