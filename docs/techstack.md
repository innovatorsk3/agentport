# Tech Stack & Build

**Ngày:** 2026-08-11
**Trạng thái:** Chốt cho v1

---

## 1. Quyết định

| Lớp | Chọn | Vì sao |
|---|---|---|
| Khung desktop | **Tauri v2** | ~5–10 MB vs ~80–150 MB của Electron. App này chỉ ghi vài file + gọi vài HTTP → không cần bundle cả Chromium. |
| UI | **React + TypeScript + Vite** | Máy đã có Node 22 + pnpm 10. UI là 3 màn hình form, không cần gì kỳ lạ. |
| Lõi | **Rust** (Tauri backend) | Chỉ dùng cho fs + HTTP + dò CLI. Bề mặt Rust rất nhỏ. |
| Đóng gói | **Portable, không installer** | App chạy 2 lần trong đời (§1 requirements) — bắt cài rồi gỡ là mâu thuẫn. |
| CI | **GitHub Actions** → Releases | Runner macOS giải bài "không build Mac trên Windows được". |

### Vì sao Tauri chứ không Electron

App làm gì: ghi file config, gọi HTTP test, dò xem CLI có trên máy không. Không nặng tính toán, không UI phức tạp.

150 MB để ghi mấy file config là lố. Đổi lại Tauri dùng **webview hệ thống** (WebView2 trên Windows, WKWebView trên macOS) nên UI có thể lệch nhau chút ít giữa hai hệ — chấp nhận được với 3 màn hình form.

### Rủi ro đã biết

**Rust ở lõi.** Máy dev hiện **chưa cài** Rust (`rustc`/`cargo` not found). Phải cài trước khi chạy `pnpm tauri dev`.

Nếu Rust làm chậm tiến độ ở tuần thứ hai → phương án lùi là Electron, đổi 15× kích thước lấy tốc độ ship. Quyết định này **có thể đảo**, không phải một chiều.

**WebView2 trên Windows cũ.** Windows 11 và Windows 10 mới có sẵn. Máy cũ có thể thiếu — Tauri có tuỳ chọn bundle kèm nếu cần.

---

## 2. Cấu trúc thư mục

```
agentport/
├── src/                      # React UI
│   ├── screens/              # 3 màn: chọn nguồn · danh sách · tổng kết
│   ├── components/
│   └── types/                # kiểu bundle dùng chung với Rust
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── detect.rs         # dò CLI có trên máy không (§10)
│   │   ├── writer/           # dịch ý định → file config (§7)
│   │   │   ├── claude.rs     # permissions.defaultMode
│   │   │   └── codex.rs      # approval_policy + sandbox_mode
│   │   ├── shell.rs          # sinh script + đăng ký 1 dòng (§5)
│   │   ├── bundle.rs         # export/import, so danh tính (§9)
│   │   └── probe.rs          # test sinh chữ thật, phân loại lỗi (§10)
│   ├── icons/
│   ├── tauri.conf.json
│   └── Cargo.toml
├── .github/workflows/release.yml
├── docs/
│   ├── requirements.md
│   └── techstack.md
├── .gitignore
└── package.json
```

Cách chia này bám thẳng vào requirements — mỗi file lõi ứng với một mục.

---

## 3. Ranh giới Rust ↔ React

**Rust làm** (đụng hệ thống):
- Đọc/ghi file config của CLI
- Dò xem `claude` / `codex` có trong `PATH` không
- Sinh script shell + đăng ký một dòng vào file khởi động
- Gọi HTTP test tới provider
- Đọc/ghi file bundle

**React làm:** hiển thị, nhập liệu, trạng thái màn hình. **Không** đụng file trực tiếp.

Lý do: mọi thao tác nguy hiểm nằm sau một bề mặt Rust hẹp, dễ soát — đặc biệt là phần ghi vào file khởi động shell (§5, đây là chỗ 7 dòng Antigravity trùng lặp sinh ra).

---

## 4. Build cục bộ

```bash
# lần đầu — máy này CHƯA có Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

pnpm install
pnpm tauri dev          # chạy dev
pnpm tauri build        # build cho HĐH hiện tại
```

Node 22.22.0 ✓ · pnpm 10.15.1 ✓ · Rust ✗ (cần cài)

---

## 5. CI → GitHub Releases

### Ma trận

| Nền tảng | Runner | Target | Ra file |
|---|---|---|---|
| macOS Apple Silicon | `macos-latest` | `aarch64-apple-darwin` | `.dmg`, `.app` |
| macOS Intel | `macos-latest` | `x86_64-apple-darwin` | `.dmg`, `.app` |
| Windows x64 | `windows-latest` | mặc định | `.exe`, `.msi` |

Dùng `tauri-apps/tauri-action@v1`. Kích hoạt bằng đẩy tag `v*`.

`fail-fast: false` — một nền tảng hỏng không huỷ các nền tảng còn lại.

### Ký tên — v1 KHÔNG ký

Chưa mua chứng chỉ. Hệ quả người dùng phải biết:

**macOS:** Gatekeeper chặn lần đầu. Mở bằng chuột phải → Open, hoặc Privacy & Security → "Open Anyway".

⚠️ **Bắt buộc dù không ký:** đặt `"signingIdentity": "-"` trong `tauri.conf.json` (ad-hoc signing). Không có nó thì bản Apple Silicon tải từ GitHub Releases bị macOS báo **"damaged"** — người dùng tưởng file hỏng, không phải cảnh báo bảo mật thường. Ad-hoc không bỏ được cảnh báo Gatekeeper, nhưng bỏ được cái "damaged" gây hiểu nhầm.

**Windows:** SmartScreen hiện "Windows protected your PC" → More info → Run anyway.

Nội bộ thì chấp nhận được. Nếu sau này phát rộng: Apple Developer Program ~$99/năm, chứng chỉ ký code Windows vài trăm đô/năm *(giá cần kiểm lại)*.

---

## 6. Bảo mật: bundle chứa key

Theo §7 requirements, bundle mang key **plaintext** — dùng nội bộ, cố ý không mã hoá.

Hệ quả với repo công khai:

- `.gitignore` chặn `*.agentport`, `*.agentport.json`, `**/agentport-bundle*`
- Đuôi file cố tình **lạ mắt**, không phải `config.json` — `git add .` không tha ai
- README phải nói thẳng: **đừng commit file bundle**
- Kho mẫu chỉ có `.example` với key giả

Người dùng đã từng để `MUST1C_API_KEY` trần trong `.zshrc` — đây không phải rủi ro lý thuyết.

---

## 7. Còn để ngỏ

- **Linux** — Tauri build được AppImage/deb, nhưng chưa có yêu cầu. Thêm sau nếu cần.
- **Bundle WebView2** cho Windows cũ — chờ có máy thật báo lỗi.
- **Tự cập nhật** — Tauri có updater. App chạy 2 lần/đời thì gần như vô nghĩa; nhiều khả năng bỏ.
