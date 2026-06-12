# Copy Time Tray

跨平台系统托盘时钟，点击复制当前时间到剪贴板。

支持 **Linux** 和 **Windows**。

## 功能

- 🕐 状态栏/任务栏托盘图标
- 📋 右键菜单：复制当前时间到剪贴板
- 📅 多种时间格式：完整时间 / 日期 / 纯数字
- 🔔 复制后桌面通知
- 🪟 Linux + Windows 跨平台

## 内存占用

| 版本 | 内存 |
|------|------|
| Python | ~42 MB |
| **Rust** | **~7 MB** |

## 依赖

### Linux
- `libgtk-3-dev`
- `libappindicator3-dev`

```bash
sudo apt install -y libgtk-3-dev libappindicator3-dev
```

### Windows
- 无需额外依赖（使用 tray-icon 原生 API）

## 编译

```bash
cargo build --release
```

编译产物：`target/release/copy-time-tray`

## 运行

```bash
./target/release/copy-time-tray
```

### Linux 开机自启 (systemd)

```bash
mkdir -p ~/.config/systemd/user
cat > ~/.config/systemd/user/copy-time-tray.service << EOF
[Unit]
Description=Copy Time Tray
After=graphical-session.target

[Service]
Type=simple
ExecStart=/path/to/copy-time-tray
Restart=always
RestartSec=3

[Install]
WantedBy=graphical-session.target
EOF

systemctl --user daemon-reload
systemctl --user enable --now copy-time-tray
```

### Windows 开机自启

将 `copy-time-tray.exe` 快捷方式放到：
```
%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\
```

## 项目结构

```
copy-time-tray/
├── src/
│   └── main.rs          # 跨平台主程序（条件编译）
├── icons/
│   └── copy-time.svg    # 时钟图标（Linux）
├── Cargo.toml
└── README.md
```

## 架构

| 平台 | 托盘 | 菜单 | 事件循环 |
|------|------|------|----------|
| Linux | libappindicator | GTK | gtk::main() |
| Windows | tray-icon | muda | 简单休眠循环 |

## License

MIT
