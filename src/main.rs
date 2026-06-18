// er-qs CLI: thin wrapper over the er_qs core library.

use er_qs::*;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    // Minimal hand-rolled flag parsing: pull out global flags, keep positionals.
    let mut save_path_override: Option<PathBuf> = None;
    let mut no_wait = false;
    let mut positional: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--save-path" => {
                i += 1;
                let p = args.get(i).ok_or("--save-path requires a value")?;
                save_path_override = Some(PathBuf::from(p));
            }
            "--no-wait" => no_wait = true,
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            other => positional.push(other.to_string()),
        }
        i += 1;
    }

    let cmd = positional.first().map(String::as_str).unwrap_or("help");
    let arg = positional.get(1).cloned();
    match cmd {
        "save" => {
            let save = locate_save(save_path_override)?;
            let slot = arg.unwrap_or_else(|| QUICK_SLOT.to_string());
            let bytes = save_snapshot(&save, &slot, !no_wait)?;
            println!("✓ snapshot '{}' saved ({})", display_slot(&slot), human_size(bytes));
            Ok(())
        }
        "load" => {
            let save = locate_save(save_path_override)?;
            let slot = arg.unwrap_or_else(|| QUICK_SLOT.to_string());
            let backup = load_snapshot(&save, &slot)?;
            println!("✓ snapshot '{}' restored into game save", display_slot(&slot));
            println!("  ⚠ 确保现在在标题画面,然后选「继续游戏」(Continue) 读取");
            println!("  previous save backed up: {}", backup.display());
            Ok(())
        }
        "list" | "ls" => cmd_list(),
        "path" => {
            println!("{}", locate_save(save_path_override)?.display());
            Ok(())
        }
        "info" => cmd_info(save_path_override),
        "items" => cmd_items(save_path_override),
        "give" => cmd_give(
            save_path_override,
            positional.get(1).cloned(),
            positional.get(2).cloned(),
            positional.get(3).cloned(),
        ),
        "help" => {
            print_help();
            Ok(())
        }
        other => Err(format!(
            "unknown command '{other}' (try: save, load, list, path, info)"
        )),
    }
}

fn cmd_list() -> Result<(), String> {
    let items = list_snapshots()?;
    if items.is_empty() {
        println!("(no snapshots yet — run `er-qs save`)");
        return Ok(());
    }
    println!("{:<20} {:>12} {:>10}", "SLOT", "AGE", "SIZE");
    for s in items {
        let age = s
            .modified
            .elapsed()
            .map(|d| fmt_relative(d.as_secs()))
            .unwrap_or_else(|_| "just now".into());
        println!("{:<20} {:>12} {:>10}", display_slot(&s.name), age, human_size(s.size));
    }
    Ok(())
}

fn cmd_info(override_path: Option<PathBuf>) -> Result<(), String> {
    let save = SaveFile::locate_and_load(override_path)?;
    println!("save : {}", save.path.display());
    println!("size : {}", human_size(save.data.len() as u64));

    if let Some(sid) = save.steam_id() {
        println!("steam: {sid}");
    }

    let chars = save.characters();
    println!("chars: {} character(s) in use", chars.len());
    for c in &chars {
        let (h, m) = c.playtime_hm();
        println!("\n  [槽{}] {}  Lv{}  ({}h{}m)", c.slot, c.name, c.level, h, m);
        let line: Vec<String> = STAT_NAMES
            .iter()
            .zip(c.stats.iter())
            .map(|(n, v)| format!("{n}{v:>2}"))
            .collect();
        println!("       {}", line.join("  "));
    }
    if chars.is_empty() {
        println!("(还没有角色 — 建个角色存盘后这里会显示名字/等级/属性/时长)");
    }

    // Section map — also demonstrates the plaintext layout.
    println!("\nsections:");
    for (i, s) in save.sections().iter().enumerate() {
        let body = section_body(&save.data, s);
        let zero = if body.is_empty() {
            0.0
        } else {
            body.iter().filter(|&&b| b == 0).count() as f64 / body.len() as f64 * 100.0
        };
        let kind = if zero > 50.0 {
            "empty"
        } else if zero > 3.0 {
            "plaintext"
        } else {
            "binary"
        };
        println!(
            "  {:2} {:12} off={:#010x} size={:>9} zero={:5.1}% [{}]",
            i, s.name, s.offset, s.size, zero, kind
        );
    }
    Ok(())
}

fn cmd_items(override_path: Option<PathBuf>) -> Result<(), String> {
    let save = SaveFile::locate_and_load(override_path)?;
    for c in save.characters() {
        let owned = save.owned_items(c.slot);
        println!("\n[槽{}] {} — 拥有 {} 种常用物品:", c.slot, c.name, owned.len());
        for (name, _id, qty) in owned {
            println!("  {name} ×{qty}");
        }
    }
    Ok(())
}

fn cmd_give(
    override_path: Option<PathBuf>,
    name: Option<String>,
    qty: Option<String>,
    slot_arg: Option<String>,
) -> Result<(), String> {
    let name = name.ok_or("用法: er-qs give <物品名关键词> <数量> [槽位]")?;
    let qty: u32 = qty
        .and_then(|s| s.parse().ok())
        .ok_or("用法: er-qs give <物品名关键词> <数量> [槽位]")?;
    let matches = item_search(&name);
    match matches.len() {
        0 => Err(format!("没找到匹配 '{name}' 的物品")),
        1 => {
            let (base_id, item_name) = matches[0];
            let save = locate_save(override_path)?;
            let mut sf = SaveFile::load(save)?;
            let chars = sf.characters();
            let slot = match slot_arg {
                Some(s) => {
                    let n: usize = s.parse().map_err(|_| "槽位要是数字(0-9)")?;
                    if !chars.iter().any(|c| c.slot == n) {
                        return Err(format!("槽{n} 没有角色"));
                    }
                    n
                }
                None => chars.first().map(|c| c.slot).ok_or("没有角色")?,
            };
            let cname = chars
                .iter()
                .find(|c| c.slot == slot)
                .map(|c| c.name.clone())
                .unwrap_or_default();
            sf.give_item(slot, base_id, qty)?;
            let backup = sf.write_back()?;
            println!("✓ 已给 [槽{slot}] {cname}: {item_name} ×{qty}(已写回存档)");
            println!("  备份: {}", backup.display());
            println!("  ⚠ 确保游戏没运行; 进游戏「继续游戏」验证物品是否出现、能否使用");
            Ok(())
        }
        _ => {
            println!("匹配多个,请用更精确的关键词:");
            for (id, n) in matches.iter().take(20) {
                println!("  {n}  (id {id})");
            }
            if matches.len() > 20 {
                println!("  … 共 {} 个匹配", matches.len());
            }
            Ok(())
        }
    }
}

fn print_help() {
    println!(
        r#"er-qs — Elden Ring 存档工具 (Linux/Proton)

USAGE:
    er-qs <COMMAND> [name] [--save-path <FILE>] [--no-wait]

COMMANDS:
    save [name]    备份当前存档到快照(不带 name = 快速槽 [quick])
    load [name]    把快照恢复到游戏存档(不带 name = 快速槽 [quick])
    list           列出所有快照
    path           显示自动定位到的存档文件路径
    info           显示存档信息(SteamID / 角色 / 段布局)
    help           显示本帮助

GUI:
    er-gui         图形界面(需用 `cargo build --features gui` 构建)

WORKFLOW (打 BOSS 前后):
    1. 雾门前站定,等右上角存档图标转完 → er-qs save
    2. 失误了 → 游戏内 quitout 回到标题画面
    3. er-qs load → 在标题画面选「继续游戏」即回到快照

FLAGS:
    --save-path <FILE>   手动指定存档文件(覆盖自动定位)
    --no-wait            跳过"等待落盘"(save 时不等 mtime 稳定)

ENV:
    ER_SAVE_PATH         手动指定存档文件(等同 --save-path)
"#
    );
}
