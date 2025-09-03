# OverTLS GUI

[English README](README.md)

OverTLS GUI 是一款基於 Rust 和 FLTK 的跨平台圖形化管理工具，
用於管理 [OverTLS](https://github.com/ShadowsocksR-Live/OverTLS) 客戶端節點。
它提供直觀的界面，支持在 Linux、Windows 和 macOS 上配置、導入和運行 OverTLS 節點。

## 功能特性

- **節點管理**：新建、導入、刪除、查看節點詳情。
- **配置導入**：支持從 JSON 文件、剪貼板或掃描屏幕上的 QR Code 導入節點配置。
- **系統設置**：可配置本地監聽、連接池、DNS 緩存、Tun2proxy 代理等。
- **日誌查看**：主窗口底部實時顯示運行日誌，支持調整日誌等級。
- **系統托盤支持**：可最小化到托盤，隨時顯示/隱藏主窗口或退出。
- **權限檢查**：Linux 下自動檢查 root 權限，必要時自動以管理員身份重啟。
- **多語言支持**：界面文本可根據系統語言自動切換（如有配置）。

## 安裝與編譯

### 依賴

- [Rust 1.85+](https://www.rust-lang.org/)

#### Linux 額外編譯依賴

在 Linux 下編譯前，需安裝以下系統庫（與 CI 配置一致）：

```bash
sudo apt-get update
sudo apt-get install --fix-missing -y libxmu-dev \
  libx11-dev libxext-dev libxft-dev libxinerama-dev libxcursor-dev libxrender-dev libxfixes-dev \
  libpango1.0-dev libgl1-mesa-dev libglu1-mesa-dev libgdk-pixbuf2.0-dev libgtk-3-dev libxdo-dev
```

### 編譯

```bash
git clone https://github.com/ShadowsocksR-Live/overtls-gui.git
cd overtls-gui
cargo build --release
```

### 運行

Linux（需 root 權限）：

```bash
sudo ./target/release/overtls-gui
```

Windows/macOS：

```bash
./target/release/overtls-gui
```

## 使用方法

- 通過菜單欄可導入節點（文件/剪貼板/QR Code）、新建、刪除、查看詳情。
- 點擊“Settings”可設置本地監聽、Tun2proxy、日誌等參數。
- 托盤圖標可隨時顯示/隱藏主窗口或退出程序。
- 節點詳情支持自定義備註、Tunnel Path、TLS、Client 參數等。

## 配置說明

- 支持多節點管理，配置文件格式兼容 OverTLS 標準。
- Tun2proxy 代理、DNS 策略、日誌等可在設置界面靈活調整。
- 支持導入/導出節點配置，便於備份和遷移。

## 截圖

![Main Window](screenshots/main_window.png)
![Details](screenshots/details.png)
![Settings](screenshots/settings.png)

## 協議

- 開源協議：MIT

---

如需更詳細的使用說明、配置格式或常見問題，請參見源碼註釋或提交 Issue。
