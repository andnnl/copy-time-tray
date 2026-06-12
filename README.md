# Copy Time Tray (Rust)

系统托盘时钟按钮，点击复制当前时间到剪贴板。Rust 版本，内存占用更低。

## 功能

- 🕐 状态栏显示时钟图标
- 📋 右键菜单：复制当前时间到剪贴板
- 📅 支持多种时间格式：
  - 完整时间：`2026-06-12 10:30:45`
  - 日期：`2026-06-12`
  - 纯数字：`20260612103045`
- 🔔 复制后弹出通知提示
- 🚀 开机自启

## 内存占用对比

| 版本 | 内存 | 二进制大小 |
|------|------|-----------|
| Python (GTK) | ~42 MB | - |
| **Rust** | **~26 MB** | **1.3 MB** |

## 依赖

### 系统依赖

```bash
sudo apt install -y libappindicator3-dev libgtk-3-dev
```

### 编译依赖

- Rust 1.70+ (通过 rustup 安装)

## 编译

```bash
cargo build --release
```

编译产物：`target/release/copy-time-tray`（1.3 MB）

## 运行

```bash
./target/release/copy-time-tray
```

或后台运行：

```bash
nohup ./target/release/copy-time-tray &
```

## 开机自启

```bash
mkdir -p ~/.config/autostart
cat > ~/.config/autostart/copy-time-tray.desktop << EOF
[Desktop Entry]
Type=Application
Name=Copy Time Tray
Comment=点击复制当前时间到剪贴板
Exec=$(pwd)/target/release/copy-time-tray
Hidden=false
NoDisplay=false
X-GNOME-Autostart-enabled=true
EOF
```

## 项目结构

```
copy-time-tray/
├── src/
│   └── main.rs          # 主程序
├── Cargo.toml           # 依赖配置
└── README.md
```

## 依赖 crate

- `libappindicator` - 系统托盘（libappindicator3 绑定）
- `gtk` - GTK3 菜单
- `chrono` - 时间处理
- `arboard` - 剪贴板操作
- `notify-rust` - 桌面通知

## 卸载

```bash
# 删除开机自启
rm ~/.config/autostart/copy-time-tray.desktop

# 停止进程
pkill -f copy-time-tray
```

## License

MIT
