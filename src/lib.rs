// er-qs core library: Elden Ring save parsing + snapshot management.
//
// Shared by the CLI (`er-qs`) and the GUI (`er-gui`). Pure std — no external
// dependencies. It only touches the .sl2 file on the Linux side; it never
// touches the game process, so there is no anti-cheat interaction.
//
// Elden Ring saves are a BND4 container that is plaintext on disk (verified:
// SteamID and character data appear unencrypted). We parse the BND4 entry table
// for reliable per-section offsets, which avoids absolute-offset drift between
// game/DLC versions.

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub mod item_db;

/// Steam app id for Elden Ring; the compatdata folder is named after it.
pub const ER_APPID: &str = "1245620";
/// Default slot used by the quick-save workflow.
pub const QUICK_SLOT: &str = "_quick";
/// How many pre-load safety backups to keep before pruning.
pub const MAX_BACKUPS: usize = 20;
/// Stat display names (official zh-Hans), in storage order.
/// Note: Arcane is 感应 in the official simplified-Chinese build (not 神秘).
pub const STAT_NAMES: [&str; 8] = ["生命力", "集中力", "耐力", "力气", "灵巧", "智力", "信仰", "感应"];

/// An editable item: display name + base id (the id WITHOUT the 0x40000000 /
/// 0xB0000000 category prefix). The inventory entry's ga_item_handle is
/// `base_id | 0xB0000000` for normal goods.
pub struct ItemDef {
    pub name: &'static str,
    pub base_id: u32,
}

/// Common runes / upgrade materials / consumables / key items. Base ids
/// cross-verified against ClayAmore/ER-Save-Editor and Ariescyn. Names are in
/// English pending an official zh-Hans pass.
pub const COMMON_ITEMS: &[ItemDef] = &[
    ItemDef { name: "黄金卢恩[1]", base_id: 2900 },
    ItemDef { name: "黄金卢恩[2]", base_id: 2901 },
    ItemDef { name: "黄金卢恩[3]", base_id: 2902 },
    ItemDef { name: "黄金卢恩[4]", base_id: 2903 },
    ItemDef { name: "黄金卢恩[5]", base_id: 2904 },
    ItemDef { name: "黄金卢恩[6]", base_id: 2905 },
    ItemDef { name: "黄金卢恩[7]", base_id: 2906 },
    ItemDef { name: "黄金卢恩[8]", base_id: 2907 },
    ItemDef { name: "黄金卢恩[9]", base_id: 2908 },
    ItemDef { name: "黄金卢恩[10]", base_id: 2909 },
    ItemDef { name: "黄金卢恩[11]", base_id: 2910 },
    ItemDef { name: "黄金卢恩[12]", base_id: 2911 },
    ItemDef { name: "黄金卢恩[13]", base_id: 2912 },
    ItemDef { name: "稀人卢恩", base_id: 2913 },
    ItemDef { name: "英雄卢恩[1]", base_id: 2914 },
    ItemDef { name: "英雄卢恩[2]", base_id: 2915 },
    ItemDef { name: "英雄卢恩[3]", base_id: 2916 },
    ItemDef { name: "英雄卢恩[4]", base_id: 2917 },
    ItemDef { name: "英雄卢恩[5]", base_id: 2918 },
    ItemDef { name: "王之卢恩", base_id: 2919 },
    ItemDef { name: "卢恩弯弧", base_id: 190 },
    ItemDef { name: "锻造石[1]", base_id: 10100 },
    ItemDef { name: "锻造石[2]", base_id: 10101 },
    ItemDef { name: "锻造石[3]", base_id: 10102 },
    ItemDef { name: "锻造石[4]", base_id: 10103 },
    ItemDef { name: "锻造石[5]", base_id: 10104 },
    ItemDef { name: "锻造石[6]", base_id: 10105 },
    ItemDef { name: "锻造石[7]", base_id: 10106 },
    ItemDef { name: "锻造石[8]", base_id: 10107 },
    ItemDef { name: "古龙岩锻造石", base_id: 10140 },
    ItemDef { name: "失色锻造石[1]", base_id: 10160 },
    ItemDef { name: "失色锻造石[2]", base_id: 10161 },
    ItemDef { name: "失色锻造石[3]", base_id: 10162 },
    ItemDef { name: "失色锻造石[4]", base_id: 10163 },
    ItemDef { name: "失色锻造石[5]", base_id: 10164 },
    ItemDef { name: "失色锻造石[6]", base_id: 10165 },
    ItemDef { name: "失色锻造石[7]", base_id: 10166 },
    ItemDef { name: "失色锻造石[8]", base_id: 10167 },
    ItemDef { name: "失色锻造石[9]", base_id: 10200 },
    ItemDef { name: "古龙岩失色锻造石", base_id: 10168 },
    ItemDef { name: "墓地铃兰[1]", base_id: 10900 },
    ItemDef { name: "墓地铃兰[2]", base_id: 10901 },
    ItemDef { name: "墓地铃兰[3]", base_id: 10902 },
    ItemDef { name: "墓地铃兰[4]", base_id: 10903 },
    ItemDef { name: "墓地铃兰[5]", base_id: 10904 },
    ItemDef { name: "墓地铃兰[6]", base_id: 10905 },
    ItemDef { name: "墓地铃兰[7]", base_id: 10906 },
    ItemDef { name: "墓地铃兰[8]", base_id: 10907 },
    ItemDef { name: "墓地铃兰[9]", base_id: 10908 },
    ItemDef { name: "大朵墓地铃兰", base_id: 10909 },
    ItemDef { name: "灵依墓地铃兰[1]", base_id: 10910 },
    ItemDef { name: "灵依墓地铃兰[2]", base_id: 10911 },
    ItemDef { name: "灵依墓地铃兰[3]", base_id: 10912 },
    ItemDef { name: "灵依墓地铃兰[4]", base_id: 10913 },
    ItemDef { name: "灵依墓地铃兰[5]", base_id: 10914 },
    ItemDef { name: "灵依墓地铃兰[6]", base_id: 10915 },
    ItemDef { name: "灵依墓地铃兰[7]", base_id: 10916 },
    ItemDef { name: "灵依墓地铃兰[8]", base_id: 10917 },
    ItemDef { name: "灵依墓地铃兰[9]", base_id: 10918 },
    ItemDef { name: "大朵灵依墓地铃兰", base_id: 10919 },
    ItemDef { name: "腐败苔药", base_id: 940 },
    ItemDef { name: "唤勾指药", base_id: 150 },
    ItemDef { name: "腌制白银鸟爪", base_id: 1190 },
    ItemDef { name: "腌制黄金鸟爪", base_id: 1200 },
    ItemDef { name: "勇者肉块", base_id: 1210 },
    ItemDef { name: "星星泪滴", base_id: 2130 },
    ItemDef { name: "泪滴幼体", base_id: 8185 },
    ItemDef { name: "黄金种子", base_id: 10010 },
    ItemDef { name: "圣杯露滴", base_id: 10020 },
    ItemDef { name: "记忆石", base_id: 10030 },
    ItemDef { name: "护符皮袋", base_id: 10040 },
    ItemDef { name: "龙心脏", base_id: 10060 },
    ItemDef { name: "死根", base_id: 2090 },
    ItemDef { name: "石剑钥匙", base_id: 8000 },
    ItemDef { name: "魔石剑钥匙", base_id: 8186 },
];

// --- public types ---------------------------------------------------------

/// One character (save slot) parsed from the file.
#[derive(Clone, Debug)]
pub struct Character {
    pub slot: usize,
    pub name: String,
    pub level: u16,
    /// vigor, mind, endurance, strength, dexterity, intelligence, faith, arcane
    pub stats: [u8; 8],
    /// current runes (souls)
    pub runes: u32,
    pub playtime_secs: u32,
}

impl Character {
    /// Playtime as (hours, minutes).
    pub fn playtime_hm(&self) -> (u32, u32) {
        (self.playtime_secs / 3600, (self.playtime_secs % 3600) / 60)
    }
}

/// A BND4 section (USER_DATA_xxx).
#[derive(Clone, Debug)]
pub struct Section {
    pub name: String,
    pub offset: usize,
    pub size: usize,
}

/// A loaded save file plus its raw bytes.
pub struct SaveFile {
    pub path: PathBuf,
    pub data: Vec<u8>,
}

impl SaveFile {
    /// Load and validate a save file from an explicit path.
    pub fn load(path: PathBuf) -> Result<SaveFile, String> {
        let data = fs::read(&path).map_err(|e| format!("read save: {e}"))?;
        if data.len() < 0x40 || &data[..4] != b"BND4" {
            return Err("not a valid BND4 .sl2 save".into());
        }
        Ok(SaveFile { path, data })
    }

    /// Auto-locate (or use the override) and load.
    pub fn locate_and_load(override_path: Option<PathBuf>) -> Result<SaveFile, String> {
        SaveFile::load(locate_save(override_path)?)
    }

    pub fn sections(&self) -> Vec<Section> {
        parse_sections(&self.data)
    }

    pub fn steam_id(&self) -> Option<u64> {
        read_steam_id(&self.data, &self.sections())
    }

    pub fn characters(&self) -> Vec<Character> {
        parse_characters(&self.data, &self.sections())
    }

    /// Recompute one section's checksum: the leading 16 bytes are MD5 of the
    /// section's data body. Must be called after editing any bytes in a section.
    fn recalc_checksum(&mut self, offset: usize, size: usize) {
        let start = offset + 16;
        let end = (offset + size).min(self.data.len());
        if start >= end {
            return;
        }
        let digest = md5::compute(&self.data[start..end]);
        self.data[offset..offset + 16].copy_from_slice(&digest.0);
    }

    /// Write the (edited) bytes back to the save file, backing up the current
    /// on-disk file first. Returns the backup path. Edits are in-memory until
    /// this is called.
    pub fn write_back(&self) -> Result<PathBuf, String> {
        let bdir = backup_dir();
        fs::create_dir_all(&bdir).map_err(|e| format!("create backup dir: {e}"))?;
        let backup = bdir.join(format!("preedit-{}.sl2", unix_now()));
        fs::copy(&self.path, &backup).map_err(|e| format!("backup failed: {e}"))?;
        prune_backups(&bdir);
        fs::write(&self.path, &self.data).map_err(|e| format!("write failed: {e}"))?;
        Ok(backup)
    }

    /// Edit a character's 8 stats. Recomputes the derived level (both the in-slot
    /// copy and the menu-header copy) and the affected section checksums.
    /// In-memory only — call `write_back()` to persist.
    pub fn set_stats(&mut self, slot: usize, new_stats: [u8; 8]) -> Result<(), String> {
        let secs = self.sections();
        let slot_sec = secs.get(slot).cloned().ok_or("invalid slot")?;
        let header = secs.get(10).cloned().ok_or("no header section")?;

        let body_off = slot_sec.offset + 16;
        let body_end = (slot_sec.offset + slot_sec.size).min(self.data.len());
        let (anchor, _, _) =
            find_stat_anchor(&self.data[body_off..body_end]).ok_or("no character in this slot")?;

        // Level is derived: sum(stats) - 79. Guard the valid range.
        let sum: u32 = new_stats.iter().map(|&x| x as u32).sum();
        let new_level = sum
            .checked_sub(79)
            .filter(|&l| (1..=713).contains(&l))
            .ok_or("resulting level out of range (stats sum must be 80..=792)")?
            as u16;

        let abs = body_off + anchor;
        // 8 stats: value in the low byte at anchor + k*4
        for (k, &v) in new_stats.iter().enumerate() {
            self.data[abs + k * 4] = v;
        }
        // in-slot level copy (u16 at anchor + 44)
        self.data[abs + 44..abs + 46].copy_from_slice(&new_level.to_le_bytes());
        // menu-header level copy (u16 at name_base + slot*stride + 34)
        let lvl_off = header.offset + 16 + NAME_BASE_IN_HEADER + slot * NAME_STRIDE + 34;
        if lvl_off + 2 <= self.data.len() {
            self.data[lvl_off..lvl_off + 2].copy_from_slice(&new_level.to_le_bytes());
        }

        // Checksums for the two sections we touched.
        self.recalc_checksum(slot_sec.offset, slot_sec.size);
        self.recalc_checksum(header.offset, header.size);
        Ok(())
    }

    /// Edit a character's name (max 16 chars). Recomputes the header checksum.
    /// In-memory only — call `write_back()` to persist.
    pub fn set_name(&mut self, slot: usize, new_name: &str) -> Result<(), String> {
        let secs = self.sections();
        let header = secs.get(10).cloned().ok_or("no header section")?;
        let name_off = header.offset + 16 + NAME_BASE_IN_HEADER + slot * NAME_STRIDE;
        if name_off + 32 > self.data.len() {
            return Err("name offset out of range".into());
        }
        let utf16: Vec<u16> = new_name.encode_utf16().collect();
        if utf16.len() > 16 {
            return Err("name too long (max 16 characters)".into());
        }
        // 32-byte fixed field, UTF-16LE, zero-padded (zeros act as terminator).
        let mut bytes = [0u8; 32];
        for (i, u) in utf16.iter().enumerate() {
            bytes[i * 2..i * 2 + 2].copy_from_slice(&u.to_le_bytes());
        }
        self.data[name_off..name_off + 32].copy_from_slice(&bytes);
        self.recalc_checksum(header.offset, header.size);
        Ok(())
    }

    /// Replace the SteamID everywhere it appears (it is embedded in several
    /// sections) and recompute all section checksums. For importing a save from
    /// another account. In-memory only — call `write_back()` to persist.
    /// Returns the number of occurrences replaced.
    pub fn set_steam_id(&mut self, new_id: u64) -> Result<usize, String> {
        let secs = self.sections();
        let old_id = read_steam_id(&self.data, &secs).ok_or("could not read current SteamID")?;
        if old_id == new_id {
            return Ok(0);
        }
        let old_le = old_id.to_le_bytes();
        let new_le = new_id.to_le_bytes();
        let mut count = 0;
        let mut i = 0;
        while i + 8 <= self.data.len() {
            if self.data[i..i + 8] == old_le {
                self.data[i..i + 8].copy_from_slice(&new_le);
                count += 1;
                i += 8;
            } else {
                i += 1;
            }
        }
        for s in &secs {
            self.recalc_checksum(s.offset, s.size);
        }
        Ok(count)
    }

    /// Set the rune count. Located via the stat anchor (souls = anchor + 48,
    /// u32 LE) — no need for the current value. In-memory only — call
    /// `write_back()` to persist.
    pub fn set_runes(&mut self, slot: usize, new_value: u32) -> Result<(), String> {
        let secs = self.sections();
        let slot_sec = secs.get(slot).cloned().ok_or("invalid slot")?;
        let body_off = slot_sec.offset + 16;
        let body_end = (slot_sec.offset + slot_sec.size).min(self.data.len());
        let (anchor, _, _) =
            find_stat_anchor(&self.data[body_off..body_end]).ok_or("no character in this slot")?;
        let souls = body_off + anchor + 48;
        if souls + 4 > self.data.len() {
            return Err("souls offset out of range".into());
        }
        self.data[souls..souls + 4].copy_from_slice(&new_value.to_le_bytes());
        self.recalc_checksum(slot_sec.offset, slot_sec.size);
        Ok(())
    }

    /// Delete a character: zero its slot data and its header name/level fields,
    /// then recompute checksums. In-memory only — call `write_back()` to persist.
    pub fn delete_character(&mut self, slot: usize) -> Result<(), String> {
        let secs = self.sections();
        let slot_sec = secs.get(slot).cloned().ok_or("invalid slot")?;
        let header = secs.get(10).cloned().ok_or("no header section")?;
        let start = slot_sec.offset + 16;
        let end = (slot_sec.offset + slot_sec.size).min(self.data.len());
        self.data[start..end].fill(0);
        let name_off = header.offset + 16 + NAME_BASE_IN_HEADER + slot * NAME_STRIDE;
        let clear_end = (name_off + 40).min(self.data.len());
        if name_off < clear_end {
            self.data[name_off..clear_end].fill(0);
        }
        self.recalc_checksum(slot_sec.offset, slot_sec.size);
        self.recalc_checksum(header.offset, header.size);
        Ok(())
    }

    /// List every goods item this character owns: (zh-Hans name, base_id, quantity).
    /// Scans the slot once for ITEM-category inventory entries and resolves names
    /// via the full item_db.
    pub fn owned_items(&self, slot: usize) -> Vec<(&'static str, u32, u32)> {
        let secs = self.sections();
        let Some(slot_sec) = secs.get(slot) else {
            return Vec::new();
        };
        let body = section_body(&self.data, slot_sec);
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        let mut i = 0;
        while i + 8 <= body.len() {
            let handle = le_u32(body, i);
            // An ITEM-category ga_item_handle has high nibble 0xB.
            if handle >> 28 == 0xB {
                let qty = le_u32(body, i + 4);
                // A real inventory quantity, not a ga_items item_id (0x4000_xxxx).
                if (1..=99_999).contains(&qty) {
                    let base_id = handle & 0x0FFF_FFFF;
                    if seen.insert(base_id) {
                        if let Some(name) = item_db::item_name(base_id) {
                            result.push((name, base_id, qty));
                        }
                    }
                }
            }
            i += 1;
        }
        result
    }

    /// Set the quantity of an item the character already owns. Locates the
    /// inventory entry by its ga_item_handle and writes the quantity (u32 at +4).
    /// In-memory only — call `write_back()` to persist.
    pub fn set_item_quantity(&mut self, slot: usize, base_id: u32, qty: u32) -> Result<(), String> {
        let secs = self.sections();
        let slot_sec = secs.get(slot).cloned().ok_or("invalid slot")?;
        let body_off = slot_sec.offset + 16;
        let body_end = (slot_sec.offset + slot_sec.size).min(self.data.len());
        let needle = (0xB000_0000u32 | base_id).to_le_bytes();
        let mut target = None;
        let mut i = body_off;
        while i + 8 <= body_end {
            if self.data[i..i + 4] == needle {
                // Inventory entry has a small quantity at +4; a ga_items entry has
                // the item_id (0x4000_xxxx) there — skip those.
                if le_u32(&self.data, i + 4) < 0x4000_0000 {
                    target = Some(i + 4);
                    break;
                }
            }
            i += 1;
        }
        let pos = target.ok_or("角色没有这个物品(当前只能修改已拥有物品的数量)")?;
        self.data[pos..pos + 4].copy_from_slice(&qty.to_le_bytes());
        self.recalc_checksum(slot_sec.offset, slot_sec.size);
        Ok(())
    }

    /// Give a character an item: if already owned, set its quantity; otherwise
    /// append a new entry to the held common_items table. Goods carry their id in
    /// the ga_item_handle, so no ga_items registration is needed (verified).
    /// In-memory only — call `write_back()` to persist.
    pub fn give_item(&mut self, slot: usize, base_id: u32, qty: u32) -> Result<(), String> {
        if self.owned_items(slot).iter().any(|&(_, b, _)| b == base_id) {
            return self.set_item_quantity(slot, base_id, qty);
        }
        let secs = self.sections();
        let slot_sec = secs.get(slot).cloned().ok_or("invalid slot")?;
        let body_off = slot_sec.offset + 16;
        let body_end = (slot_sec.offset + slot_sec.size).min(self.data.len());
        let (anchor, _, _) =
            find_stat_anchor(&self.data[body_off..body_end]).ok_or("no character in this slot")?;
        // common_distinct = PGD_start + 0x3A4; PGD_start = souls_abs - 0x64; souls = anchor + 48.
        let cd_off = body_off + anchor + 48 - 0x64 + 0x3A4;
        if cd_off + 0x9010 > self.data.len() {
            return Err("inventory table out of range".into());
        }
        let distinct = le_u32(&self.data, cd_off);
        if distinct >= 0xA80 {
            return Err("背包已满".into());
        }
        let nei_off = cd_off + 0x9008; // next_equip_index
        let nas_off = cd_off + 0x900C; // next_acquisition_sort_id
        let nei = le_u32(&self.data, nei_off);
        let nas = le_u32(&self.data, nas_off);
        let entry = cd_off + 4 + distinct as usize * 12; // common_items[distinct]
        let handle = 0xB000_0000 | base_id;
        self.data[entry..entry + 4].copy_from_slice(&handle.to_le_bytes());
        self.data[entry + 4..entry + 8].copy_from_slice(&qty.to_le_bytes());
        self.data[entry + 8..entry + 12].copy_from_slice(&nas.to_le_bytes());
        self.data[cd_off..cd_off + 4].copy_from_slice(&(distinct + 1).to_le_bytes());
        self.data[nei_off..nei_off + 4].copy_from_slice(&(nei + 1).to_le_bytes());
        self.data[nas_off..nas_off + 4].copy_from_slice(&(nas + 1).to_le_bytes());
        self.recalc_checksum(slot_sec.offset, slot_sec.size);
        Ok(())
    }
}

/// Metadata for one snapshot file.
#[derive(Clone)]
pub struct SnapshotInfo {
    pub name: String,
    pub modified: SystemTime,
    pub size: u64,
}

// --- save file location ---------------------------------------------------

pub fn locate_save(override_path: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(p) = override_path {
        return Ok(p);
    }
    if let Ok(p) = env::var("ER_SAVE_PATH") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Ok(pb);
        }
    }

    let home = env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    // Steam may live under any of these roots; some are symlinks to each other,
    // so we canonicalize and dedupe.
    let bases = [
        format!("{home}/.local/share/Steam"),
        format!("{home}/.steam/steam"),
        format!("{home}/.steam/root"),
    ];

    let mut seen = HashSet::new();
    let mut found: Vec<PathBuf> = Vec::new();
    for base in &bases {
        let er_dir = PathBuf::from(format!(
            "{base}/steamapps/compatdata/{ER_APPID}/pfx/drive_c/users/steamuser/AppData/Roaming/EldenRing"
        ));
        if !er_dir.is_dir() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&er_dir) {
            for e in entries.flatten() {
                let cand = e.path().join("ER0000.sl2");
                if cand.is_file() {
                    if let Ok(canon) = cand.canonicalize() {
                        if seen.insert(canon.clone()) {
                            found.push(canon);
                        }
                    }
                }
            }
        }
    }

    match found.len() {
        0 => Err("could not auto-locate ER0000.sl2; set ER_SAVE_PATH or pass --save-path".into()),
        _ => Ok(found.remove(0)),
    }
}

// --- snapshot management --------------------------------------------------

pub fn data_dir() -> PathBuf {
    let base = env::var("XDG_DATA_HOME")
        .unwrap_or_else(|_| format!("{}/.local/share", env::var("HOME").unwrap_or_default()));
    PathBuf::from(base).join("er-qs")
}

pub fn snapshot_dir() -> PathBuf {
    data_dir().join("snapshots")
}

pub fn backup_dir() -> PathBuf {
    data_dir().join("backups")
}

/// Copy the current save into a snapshot. Returns the byte count.
/// When `wait` is set, waits for the game's autosave writes to settle first.
pub fn save_snapshot(save_path: &Path, slot: &str, wait: bool) -> Result<u64, String> {
    validate_slot(slot)?;
    let dir = snapshot_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("create snapshot dir: {e}"))?;
    let dest = dir.join(format!("{slot}.sl2"));
    if wait {
        wait_until_stable(save_path);
    }
    fs::copy(save_path, &dest).map_err(|e| format!("copy failed: {e}"))
}

/// Restore a snapshot into the game save. Backs up the current save first and
/// returns the backup path.
pub fn load_snapshot(save_path: &Path, slot: &str) -> Result<PathBuf, String> {
    validate_slot(slot)?;
    let src = snapshot_dir().join(format!("{slot}.sl2"));
    if !src.is_file() {
        return Err(format!("snapshot '{}' not found ({})", display_slot(slot), src.display()));
    }
    let bdir = backup_dir();
    fs::create_dir_all(&bdir).map_err(|e| format!("create backup dir: {e}"))?;
    let backup = bdir.join(format!("preload-{}.sl2", unix_now()));
    fs::copy(save_path, &backup).map_err(|e| format!("backup failed: {e}"))?;
    prune_backups(&bdir);
    fs::copy(&src, save_path).map_err(|e| format!("restore failed: {e}"))?;
    Ok(backup)
}

/// List all snapshots, newest first.
pub fn list_snapshots() -> Result<Vec<SnapshotInfo>, String> {
    let dir = snapshot_dir();
    let mut items = Vec::new();
    if dir.is_dir() {
        for e in fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("sl2") {
                continue;
            }
            let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("?").to_string();
            let meta = e.metadata().map_err(|e| e.to_string())?;
            items.push(SnapshotInfo {
                name,
                modified: meta.modified().unwrap_or(UNIX_EPOCH),
                size: meta.len(),
            });
        }
    }
    items.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(items)
}

/// Delete a snapshot by name.
pub fn delete_snapshot(slot: &str) -> Result<(), String> {
    validate_slot(slot)?;
    let p = snapshot_dir().join(format!("{slot}.sl2"));
    fs::remove_file(&p).map_err(|e| format!("delete failed: {e}"))
}

// Poll the save file's mtime and return once it has been quiet for a short
// window, or after a hard timeout.
fn wait_until_stable(path: &Path) {
    let quiet = Duration::from_millis(1500);
    let timeout = Duration::from_secs(5);
    let start = SystemTime::now();
    let mut last_mtime = mtime(path);
    let mut last_change = SystemTime::now();
    loop {
        sleep(Duration::from_millis(250));
        let m = mtime(path);
        if m != last_mtime {
            last_mtime = m;
            last_change = SystemTime::now();
        }
        if last_change.elapsed().unwrap_or_default() >= quiet {
            break;
        }
        if start.elapsed().unwrap_or_default() >= timeout {
            break;
        }
    }
}

fn mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn prune_backups(dir: &Path) {
    let mut backups: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("sl2"))
        .collect();
    if backups.len() <= MAX_BACKUPS {
        return;
    }
    backups.sort_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok());
    let remove = backups.len() - MAX_BACKUPS;
    for p in backups.into_iter().take(remove) {
        let _ = fs::remove_file(p);
    }
}

// --- BND4 / character parsing ---------------------------------------------

fn parse_sections(data: &[u8]) -> Vec<Section> {
    let num = le_u32(data, 0x0C) as usize;
    let mut secs = Vec::with_capacity(num);
    for i in 0..num {
        let base = 0x40 + i * 0x20;
        if base + 0x20 > data.len() {
            break;
        }
        let size = le_u64(data, base + 8) as usize;
        let offset = le_u32(data, base + 0x10) as usize;
        let noff = le_u32(data, base + 0x14) as usize;
        secs.push(Section {
            name: read_bnd4_name(data, noff),
            offset,
            size,
        });
    }
    secs
}

/// Section body, skipping the 16-byte checksum header, bounds-checked.
pub fn section_body<'a>(data: &'a [u8], s: &Section) -> &'a [u8] {
    let end = (s.offset + s.size).min(data.len());
    let start = (s.offset + 16).min(end);
    &data[start..end]
}

fn read_bnd4_name(data: &[u8], mut off: usize) -> String {
    let mut units = Vec::new();
    while off + 1 < data.len() {
        let c = u16::from_le_bytes([data[off], data[off + 1]]);
        if c == 0 {
            break;
        }
        units.push(c);
        off += 2;
    }
    String::from_utf16_lossy(&units)
}

// SteamID lives in the menu/header section (USER_DATA_010), right after its
// 16-byte checksum header. Sanity-checked against the steam64 high dword.
fn read_steam_id(data: &[u8], secs: &[Section]) -> Option<u64> {
    let s = secs.get(10)?;
    let base = s.offset + 16 + 4;
    if base + 8 > data.len() {
        return None;
    }
    let id = le_u64(data, base);
    if (id >> 32) as u32 == 0x0110_0001 {
        Some(id)
    } else {
        None
    }
}

// Stats + level use a self-calibrating anchor scan inside the slot body
// (sum(8 stats) == level + 79, with level == u16 at +44), immune to version/DLC
// offset drift. Name/playtime come from the header section via a section-relative
// offset (the 588-byte stride between name slots holds across versions).
const NAME_BASE_IN_HEADER: usize = 0x195E; // calibrated against real saves
const NAME_STRIDE: usize = 588;

fn parse_characters(data: &[u8], secs: &[Section]) -> Vec<Character> {
    let mut chars = Vec::new();
    let header = match secs.get(10) {
        Some(s) => s,
        None => return chars,
    };
    let name_base = header.offset + 16 + NAME_BASE_IN_HEADER;
    for slot in 0..10 {
        let slot_sec = match secs.get(slot) {
            Some(s) => s,
            None => continue,
        };
        let body = section_body(data, slot_sec);
        let (anchor, level, stats) = match find_stat_anchor(body) {
            Some(v) => v,
            None => continue, // empty slot
        };
        // souls (current runes) sits at anchor + 48, u32 LE
        let runes = if anchor + 52 <= body.len() {
            le_u32(body, anchor + 48)
        } else {
            0
        };
        let np = name_base + slot * NAME_STRIDE;
        let name = read_utf16_fixed(data, np, 32);
        let playtime_secs = if np + 42 <= data.len() {
            le_u32(data, np + 38)
        } else {
            0
        };
        chars.push(Character {
            slot,
            name,
            level,
            stats,
            runes,
            playtime_secs,
        });
    }
    chars
}

// Returns (anchor_offset_in_body, level, stats).
fn find_stat_anchor(body: &[u8]) -> Option<(usize, u16, [u8; 8])> {
    let limit = body.len().saturating_sub(46).min(300_000);
    for i in 0..limit {
        let level = u16::from_le_bytes([body[i + 44], body[i + 45]]);
        if !(1..=713).contains(&level) {
            continue;
        }
        let stats = [
            body[i],
            body[i + 4],
            body[i + 8],
            body[i + 12],
            body[i + 16],
            body[i + 20],
            body[i + 24],
            body[i + 28],
        ];
        let sum: u32 = stats.iter().map(|&x| x as u32).sum();
        if sum == level as u32 + 79 {
            return Some((i, level, stats));
        }
    }
    None
}

fn read_utf16_fixed(data: &[u8], off: usize, max_bytes: usize) -> String {
    let mut units = Vec::new();
    let mut o = off;
    let end = (off + max_bytes).min(data.len());
    while o + 1 < end {
        let c = u16::from_le_bytes([data[o], data[o + 1]]);
        if c == 0 {
            break;
        }
        units.push(c);
        o += 2;
    }
    String::from_utf16_lossy(&units)
}

fn le_u32(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}

fn le_u64(d: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(d[o..o + 8].try_into().unwrap())
}

// --- small formatting helpers (shared by CLI + GUI) -----------------------

pub fn validate_slot(slot: &str) -> Result<(), String> {
    if slot.is_empty() || slot.contains('/') || slot.contains("..") {
        return Err(format!("invalid slot name '{slot}'"));
    }
    Ok(())
}

pub fn display_slot(slot: &str) -> String {
    if slot == QUICK_SLOT {
        "[quick]".to_string()
    } else {
        slot.to_string()
    }
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn fmt_relative(secs: u64) -> String {
    match secs {
        0..=59 => format!("{secs}s ago"),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86399 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86400),
    }
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut f = bytes as f64;
    let mut i = 0;
    while f >= 1024.0 && i < UNITS.len() - 1 {
        f /= 1024.0;
        i += 1;
    }
    format!("{f:.1} {}", UNITS[i])
}

/// Search the item_db by name substring. Returns (base_id, zh-Hans name).
pub fn item_search(keyword: &str) -> Vec<(u32, &'static str)> {
    item_db::ITEM_DB
        .iter()
        .filter(|(_, name)| name.contains(keyword))
        .map(|&(id, name)| (id, name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Edit stats -> write back -> reload, all on a COPY of the real save in the
    // temp dir. The real save file is never touched. Skips if no save/character.
    #[test]
    fn edit_stats_roundtrip() {
        let real = match locate_save(None) {
            Ok(p) => p,
            Err(_) => return,
        };
        let tmp = std::env::temp_dir().join("er_qs_edit_test.sl2");
        std::fs::copy(&real, &tmp).expect("copy to temp");

        let mut sf = SaveFile::load(tmp.clone()).expect("load temp");
        let slot = match sf.characters().first() {
            Some(c) => c.slot,
            None => {
                let _ = std::fs::remove_file(&tmp);
                return;
            }
        };

        let new_stats = [60u8, 30, 40, 25, 20, 9, 8, 45];
        sf.set_stats(slot, new_stats).expect("set_stats");
        sf.write_back().expect("write_back");

        let reloaded = SaveFile::load(tmp.clone()).expect("reload");
        let c = reloaded
            .characters()
            .into_iter()
            .find(|c| c.slot == slot)
            .expect("character still present");
        assert_eq!(c.stats, new_stats, "stats not persisted");
        let expect_level = new_stats.iter().map(|&x| x as u16).sum::<u16>() - 79;
        assert_eq!(c.level, expect_level, "level not recomputed");

        // Checksum self-consistency for the edited slot section.
        let secs = reloaded.sections();
        let s = &secs[slot];
        let stored = &reloaded.data[s.offset..s.offset + 16];
        let calc = md5::compute(&reloaded.data[s.offset + 16..s.offset + s.size]);
        assert_eq!(stored, &calc.0[..], "section checksum mismatch");

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn edit_name_roundtrip() {
        let real = match locate_save(None) {
            Ok(p) => p,
            Err(_) => return,
        };
        let tmp = std::env::temp_dir().join("er_qs_name_test.sl2");
        std::fs::copy(&real, &tmp).expect("copy to temp");
        let mut sf = SaveFile::load(tmp.clone()).expect("load");
        let slot = match sf.characters().first() {
            Some(c) => c.slot,
            None => {
                let _ = std::fs::remove_file(&tmp);
                return;
            }
        };
        sf.set_name(slot, "Tarnished").expect("set_name");
        sf.write_back().expect("write_back");
        let reloaded = SaveFile::load(tmp.clone()).expect("reload");
        let c = reloaded
            .characters()
            .into_iter()
            .find(|c| c.slot == slot)
            .expect("char");
        assert_eq!(c.name, "Tarnished", "name not persisted");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn edit_steam_id_roundtrip() {
        let real = match locate_save(None) {
            Ok(p) => p,
            Err(_) => return,
        };
        let tmp = std::env::temp_dir().join("er_qs_steam_test.sl2");
        std::fs::copy(&real, &tmp).expect("copy to temp");
        let mut sf = SaveFile::load(tmp.clone()).expect("load");
        let old = match sf.steam_id() {
            Some(id) => id,
            None => {
                let _ = std::fs::remove_file(&tmp);
                return;
            }
        };
        let new_id = old ^ 1; // flip a low bit; keeps the steam64 high dword valid
        let n = sf.set_steam_id(new_id).expect("set_steam_id");
        assert!(n > 0, "no SteamID occurrences replaced");
        sf.write_back().expect("write_back");
        let reloaded = SaveFile::load(tmp.clone()).expect("reload");
        assert_eq!(reloaded.steam_id(), Some(new_id), "SteamID not persisted");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn edit_runes_roundtrip() {
        let real = match locate_save(None) {
            Ok(p) => p,
            Err(_) => return,
        };
        let tmp = std::env::temp_dir().join("er_qs_runes_test.sl2");
        std::fs::copy(&real, &tmp).expect("copy");
        let mut sf = SaveFile::load(tmp.clone()).expect("load");
        let slot = match sf.characters().first() {
            Some(c) => c.slot,
            None => {
                let _ = std::fs::remove_file(&tmp);
                return;
            }
        };
        sf.set_runes(slot, 9_999_999).expect("set_runes");
        sf.write_back().expect("write");
        let reloaded = SaveFile::load(tmp.clone()).expect("reload");
        let c = reloaded
            .characters()
            .into_iter()
            .find(|c| c.slot == slot)
            .expect("char");
        assert_eq!(c.runes, 9_999_999, "runes not persisted");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn delete_character_roundtrip() {
        let real = match locate_save(None) {
            Ok(p) => p,
            Err(_) => return,
        };
        let tmp = std::env::temp_dir().join("er_qs_delete_test.sl2");
        std::fs::copy(&real, &tmp).expect("copy");
        let mut sf = SaveFile::load(tmp.clone()).expect("load");
        let chars = sf.characters();
        let slot = match chars.first() {
            Some(c) => c.slot,
            None => {
                let _ = std::fs::remove_file(&tmp);
                return;
            }
        };
        let before = chars.len();
        sf.delete_character(slot).expect("delete");
        sf.write_back().expect("write");
        let reloaded = SaveFile::load(tmp.clone()).expect("reload");
        let after = reloaded.characters();
        assert!(
            after.iter().all(|c| c.slot != slot),
            "slot still present after delete"
        );
        assert_eq!(after.len(), before - 1);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn edit_item_quantity_roundtrip() {
        let real = match locate_save(None) {
            Ok(p) => p,
            Err(_) => return,
        };
        let tmp = std::env::temp_dir().join("er_qs_item_test.sl2");
        std::fs::copy(&real, &tmp).expect("copy");
        let mut sf = SaveFile::load(tmp.clone()).expect("load");
        let slot = match sf.characters().first() {
            Some(c) => c.slot,
            None => {
                let _ = std::fs::remove_file(&tmp);
                return;
            }
        };
        let owned = sf.owned_items(slot);
        if owned.is_empty() {
            // character owns none of our known items — nothing to round-trip
            let _ = std::fs::remove_file(&tmp);
            return;
        }
        let base_id = owned[0].1;
        sf.set_item_quantity(slot, base_id, 777).expect("set qty");
        sf.write_back().expect("write");
        let reloaded = SaveFile::load(tmp.clone()).expect("reload");
        let q = reloaded
            .owned_items(slot)
            .into_iter()
            .find(|(_, b, _)| *b == base_id)
            .map(|(_, _, q)| q);
        assert_eq!(q, Some(777), "item quantity not persisted");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn give_item_roundtrip() {
        let real = match locate_save(None) {
            Ok(p) => p,
            Err(_) => return,
        };
        let tmp = std::env::temp_dir().join("er_qs_give_test.sl2");
        std::fs::copy(&real, &tmp).expect("copy");
        let mut sf = SaveFile::load(tmp.clone()).expect("load");
        let slot = match sf.characters().first() {
            Some(c) => c.slot,
            None => {
                let _ = std::fs::remove_file(&tmp);
                return;
            }
        };
        // Pick an item the character does NOT already own.
        let owned: HashSet<u32> = sf.owned_items(slot).iter().map(|&(_, b, _)| b).collect();
        let new_id = match item_db::ITEM_DB.iter().map(|&(id, _)| id).find(|id| !owned.contains(id)) {
            Some(id) => id,
            None => {
                let _ = std::fs::remove_file(&tmp);
                return;
            }
        };
        let before = sf.owned_items(slot).len();
        sf.give_item(slot, new_id, 50).expect("give_item");
        sf.write_back().expect("write");
        let reloaded = SaveFile::load(tmp.clone()).expect("reload");
        let owned_after = reloaded.owned_items(slot);
        let q = owned_after.iter().find(|&&(_, b, _)| b == new_id).map(|&(_, _, q)| q);
        assert_eq!(q, Some(50), "new item not present after give");
        assert_eq!(owned_after.len(), before + 1, "owned count did not grow by 1");
        let _ = std::fs::remove_file(&tmp);
    }
}
