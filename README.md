# er-save-editor — 艾尔登法环存档工坊 (Linux / Proton)

一个零运行时依赖的 Rust 工具,给 Linux 上用 Proton 跑的《艾尔登法环》做 **快速存档 + 存档查看 + 存档编辑**。
完全不碰游戏进程,只操作 `ER0000.sl2` 文件 —— 无反作弊风险、不随游戏版本失效、恢复出的是 100% 一致的存档。

## 三个前端(共享同一 `er_qs` 核心库)

| 命令 | 说明 | 构建 |
|---|---|---|
| `er` | **TUI 主力界面**(ratatui) | `--features tui` |
| `er-qs` | CLI(脚本 / Hyprland 热键用) | 默认 |
| `er-gui` | GUI(egui,可选) | `--features gui` |

## 功能

- **快速存档**(模拟器式 save state,打 BOSS 练习):存 / 读 / 命名快照,读档前自动备份。
- **存档查看**:角色名 / 等级 / 八属性 / 当前符文 / 游戏时长 / 拥有的物品(官方简中名,1700+ 可识别)。
- **存档编辑**(写回带自动备份):改符文(卢恩)、改物品数量、**加全新物品**、改名、改属性、SteamID 替换、删除角色。

## 安装

```bash
git clone https://github.com/HankunYu/er-save-editor
cd er-save-editor
cargo build --release --features tui    # er + er-qs
cargo build --release --features gui     # er-gui (可选)
./install.sh                             # 装到 ~/.local/bin(无预编译则自动 cargo build)
```

## 用法

### TUI (`er`)

```
Tab           切换 角色 / 快照 焦点
↑↓ / j k      选择
角色焦点:     g 改卢恩 · i 进物品 · R 改名
快照焦点:     Enter 读 · d 删
物品界面(i):  ↑↓ 选 · Enter 改数量 · a 加物品 · Esc 返回
加物品(a):    打字实时过滤 · ↑↓ 选 · Enter 输数量
全局:         s 快速存 · l 快速读 · n 命名存 · r 刷新 · q 退出
```

### CLI (`er-qs`)

```
er-qs save [name]              备份快照(不带 name = 快速槽)
er-qs load [name]              恢复快照
er-qs list                     列出快照
er-qs info                     角色信息(等级/属性/符文)
er-qs items                    列出拥有的物品
er-qs give <名> <数> [槽]      给角色加物品(名字模糊匹配,不带槽=槽0)
er-qs path                     显示存档路径
```

### Hyprland 热键(游戏中快速存读)

```ini
# ~/.config/hypr/hyprland.conf
bind = SUPER, F5, exec, er-qs save
bind = SUPER, F9, exec, er-qs load
```

## ⚠️ 注意

- **编辑前确保游戏没运行 / 停在主菜单**,否则游戏内存盘会盖掉编辑。
- 读档 / 读快照后,在游戏**标题画面选「继续游戏」**才生效。
- 所有编辑 / 读档前都会自动备份到 `~/.local/share/er-qs/backups/`(`preload-*` / `preedit-*`)。
- 加物品目前支持**道具类**(消耗品 / 素材 / 卢恩道具 / 贵重物品等);武器/防具暂不支持。

## 技术说明

- 艾尔登法环存档是**明文** BND4(非 AES 加密)。
- 角色数据靠**自校验属性锚点**(`sum(8属性)==等级+79`)定位,免疫游戏/DLC 版本偏移漂移。
- 当前符文在锚点 `+48`(u32 LE);物品条目 12 字节(`ga_item_handle` / `quantity` / `inventory_index`)。
- **加物品**:往背包(`EquipInventoryData`)的定长数组空槽插入条目 + 更新计数器;道具的 handle 自带 ID,无需登记 `ga_items`。
- 编辑写回时重算每段 MD5 校验和。
- 物品官方简中名源自游戏 `item.msgbnd.dcx`(zhocn FMG);物品 ID 表与 ClayAmore/ER-Save-Editor + Ariescyn EldenRing-Save-Manager 交叉验证。

## 数据位置

- 快照:`~/.local/share/er-qs/snapshots/`
- 备份:`~/.local/share/er-qs/backups/`(读档前 `preload-*`、编辑前 `preedit-*`)

## 协议

[MIT](LICENSE) © 2026 HankunYu
