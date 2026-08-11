# Agent CLI Profile Installer — Requirements

**Ngày:** 2026-08-11
**Nguồn:** Party-mode requirements discovery (Mary, John, Sally, Winston, Amelia, Paige)
**Trạng thái:** Đã chốt phạm vi — sẵn sàng dựng

---

## 1. Nó là gì

> Một **installer có giao diện** để mang cấu hình agent CLI (Claude Code, Codex) từ máy này sang máy khác — kể cả khác hệ điều hành — bao gồm cả những thứ khó chịu như bypass permission, và chứng minh nó chạy trước khi bạn rời máy.

**Không phải** app quản lý alias. Alias chỉ là bề mặt; phần ruột là **tính di động** và **bằng chứng**.

**Vòng đời:** Mở → nhập tay hoặc import bundle → ghi config xuống máy → test → đóng.
Terminal chạy độc lập sau đó, **kể cả khi gỡ app**. Mở lại app chỉ khi cần đổi thứ gì đó.

### Chỉ số thành công

Từ lúc mở app trên máy trắng đến lúc gõ `cht` chạy được: **≤ 2 phút**.
(Hiện tại làm tay: cả một buổi, với 4 lỗi im lặng trên đường.)

---

## 2. Vì sao — bằng chứng, không phải giả định

Mọi requirement lớn dưới đây đều rút từ **lỗi thật** trong một phiên debug ngày 2026-08-11:

| # | Lỗi thật đã xảy ra | Dẫn tới requirement |
|---|---|---|
| 1 | Switch profile ghi đè `~/.claude/settings.json` → dính **toàn máy**, người dùng phát hiện chứ không phải công cụ | §5 Ranh giới sở hữu |
| 2 | `defaultMode` đặt ở top-level thay vì trong `permissions` → **im lặng bị bỏ qua**, không một dòng lỗi | §7 Bundle lưu ý định · §8 Dangerous là thang bậc |
| 3 | Hàm `cp_codex` hardcode `MUST1C_CSE_API_KEY` → profile thứ hai nạp key vào sai biến | §6 Mỗi profile một biến env riêng |
| 4 | Key `cse` trả **200 OK** ở `/v1/models` nhưng **timeout** ở `/v1/responses` | §10 Test phải gọi sinh chữ thật |
| 5 | **7 dòng** `# Added by Antigravity` trùng lặp trong `.zshrc` (+ 2 dòng `postgresql@17`, 2 khối `NVM_DIR`) | §5 Một dòng bất biến · §9 Không đẻ bản sao |

**Ba trên bốn lỗi đầu là lỗi im lặng** — không crash, không log. Cứ tưởng chạy.

---

## 3. Phạm vi

### Trong phạm vi
- Nhập tay profile mới (lần đầu dùng)
- Export bundle (chọn được cái nào đi)
- Import bundle (cross-OS: macOS ↔ Windows ↔ Linux)
- Sinh script shell + đăng ký một dòng vào file khởi động shell
- Phát hiện CLI đã cài hay chưa
- Test kết nối thật, phân loại lỗi

### Ngoài phạm vi
- **Cài CLI hộ** — chỉ phát hiện và báo. Trở thành trình quản lý gói là hố không đáy.
- **Mã hoá / passphrase cho bundle** — dùng nội bộ, key đi theo dạng đọc được.
- **Dashboard thường trú / poll trạng thái** — đốt credit người dùng; test theo yêu cầu thôi.
- **Sửa cấu hình mặc định của CLI** (xem §4 tầng 1).

---

## 4. Mô hình ba tầng

```
Tầng 1 — Cấu hình mặc định của CLI     `claude` / `codex` gõ trần
Tầng 2 — Profile có tên                 htmustc, htcse
Tầng 3 — Alias                          cht, co-ht, c
```

**App chỉ chạm tầng 2 và 3.**

**Tầng 1** — app chỉ được **đặt tên gọi** (tạo alias `c` → `claude`), tuyệt đối không sửa nội dung. Không đi theo bundle.

> `alias c='claude'` ← app tạo cái này
> `claude` ← app không đụng

App sở hữu **cái tên**, không sở hữu **cái được gọi**. Xoá alias thì `claude` vẫn chạy.

**Hệ quả cần báo người dùng:** trên máy mới, `codex` gõ trần sẽ không chạy nếu nó phụ thuộc env var ngoài profile (ví dụ `MUST1C_API_KEY` đang nằm trần trong `.zshrc`). Xem §10 màn hình cuối.

---

## 5. Ranh giới sở hữu (bất khả xâm phạm)

| App sở hữu | App KHÔNG ĐỤNG |
|---|---|
| Profile có tên | Auth Anthropic của người dùng |
| Alias trỏ tới chúng | `~/.claude/settings.json` mặc định |
| Thư mục riêng của app | `~/.codex/config.toml` gốc |
| | Env var có sẵn trong shell rc |

### Không sở hữu file khởi động shell

App ghi vào **thư mục riêng**, sinh ra **một file duy nhất**. File khởi động shell chỉ nhận **đúng một dòng, một lần, không bao giờ đổi**:

```sh
# macOS/Linux — ~/.zshrc | ~/.bashrc | fish config
[ -f ~/.<app>/profiles.sh ] && . ~/.<app>/profiles.sh
```
```powershell
# Windows — $PROFILE
if (Test-Path ~/.<app>/profiles.ps1) { . ~/.<app>/profiles.ps1 }
```

**Lý do:** GUI cắm tay vào file khởi động shell bằng regex là súng chĩa vào chân — ghi hỏng một lần thì **không mở nổi terminal**, mà cần terminal để sửa. Gỡ app = xoá một dòng. Bán kính nổ bằng 0.

**Phải kiểm tra dòng đó đã tồn tại chưa trước khi thêm.** Đây chính là lỗi tạo ra 7 dòng Antigravity trùng lặp.

### File sinh ra
- Header rõ: `# GENERATED — DO NOT EDIT`
- **Phải chịu được việc người dùng vẫn sửa tay** (đã xảy ra: `alias c='claude'` được thêm tay vào giữa block)
- Đọc lại trước khi ghi, không cache mù — Claude tự ghi ngược vào `settings.json` khi người dùng dùng `/config` đổi model

---

## 6. Cơ chế: sinh script shell, KHÔNG sinh binary

**Quyết định đã đảo chiều** (Winston rút lại khuyến nghị launcher binary sau khi biết app chỉ chạy một lần/máy):

| | Sinh script shell ✅ | Launcher binary ❌ |
|---|---|---|
| Chi phí viết cho nhiều shell | Trả **một lần** (app chạy 1 lần/máy) | Không có |
| Ký Gatekeeper / SmartScreen | Không cần | Trả **mãi mãi** |
| Quản lý `PATH` | Không cần | Trả mãi mãi |
| App gỡ đi rồi | Alias **vẫn chạy** | Lệnh chết |
| Người dùng đọc/sửa/xoá được | Có | Không |

**Lời hứa:** app đi rồi, thứ nó làm vẫn còn.

### Mỗi profile một biến env riêng
Tên biến env cho key phải **lưu trong profile**, không hardcode. Đây là lỗi #3: `cp_codex` hardcode `MUST1C_CSE_API_KEY` khiến profile thứ hai nạp key vào sai biến.

### Ràng buộc tên alias
- **Không dấu cách**, không ký tự cần escape — alias là thứ **gõ ở terminal**, khác tên file (chỉ để nhìn)
- Gạch ngang OK (`cht-cse` đã chạy tốt)
- Cảnh báo nếu trùng lệnh có sẵn trên hệ thống (`cd`, `ls`…)
- PowerShell: alias **không nhận tham số** → phải sinh `function` rồi `Set-Alias`

---

## 7. Bundle: lưu Ý ĐỊNH, không lưu FILE

**Nguyên tắc trung tâm để cross-OS chạy được.**

Bundle **không chứa**:
- Đường dẫn tuyệt đối (`~/.claude/profiles/htcse.json` là khái niệm macOS; Windows là `C:\Users\...`)
- JSON của Claude hay TOML của Codex nguyên si

Bundle **chứa ý định**:
> *"Profile tên `htcse`, provider htmustc, base URL này, loại CLI Claude, bypass permission bật, model mapping kia, key này."*

Máy đích **tự dịch** ra file theo schema của phiên bản CLI đang cài.

**Lý do:** Claude vừa đổi vị trí `defaultMode` (lỗi #2) và tốn nửa buổi debug. Nếu bundle khoá vào schema của CLI, mọi bundle cũ chết khi CLI đổi. Với lớp dịch, chỉ cần sửa lớp dịch.

### Yêu cầu khác
- **Số phiên bản bundle** — định dạng sẽ đổi, bundle cũ vẫn nằm đâu đó
- **Key đi theo, đọc được** — dùng nội bộ, không mã hoá, không passphrase
- **Tên file khó lọt git** — không đặt `config.json`; dùng đuôi lạ mắt (ví dụ `.agentprofiles`). `git add .` không tha ai; người dùng đã từng để key trần trong `.zshrc`
- **Export chọn được** cái nào đi, không bắt buộc tất cả

---

## 8. Dangerous là THANG BẬC, không phải công tắc

Không phải một checkbox. Phải **dịch riêng cho từng CLI**, và mỗi CLI có số nấc khác nhau.

### Claude Code — `~/.claude/settings.json`
```json
{
  "permissions": { "defaultMode": "bypassPermissions" },
  "skipDangerousModePermissionPrompt": true
}
```
⚠️ **`defaultMode` phải nằm TRONG `permissions`.** Đặt ở top-level → im lặng bị bỏ qua, không báo lỗi (lỗi #2).

Các mức: `default` · `acceptEdits` · `plan` · `dontAsk` · `bypassPermissions`

### Codex — `~/.codex/<profile>.config.toml`
**Hai trục độc lập:**
```toml
approval_policy = "never"            # untrusted | on-request | never
sandbox_mode = "danger-full-access"  # read-only | workspace-write | danger-full-access
```
Codex có mức trung gian `workspace-write` (không hỏi, nhưng chặn ghi ngoài workspace) mà Claude **không có** tương đương.

---

## 9. Import: so DANH TÍNH, không so tên

### Hai trục nhận dạng

- **Danh tính** = provider + base URL + loại CLI
- **Tên gọi** = alias

Chúng có thể lệch nhau theo cả hai chiều:

| Danh tính | Tên | Xử lý |
|---|---|---|
| Giống | Giống | **Giống hệt** → im lặng bỏ qua, KHÔNG đẻ bản sao |
| Giống | Khác | Cùng provider, máy này gọi tên khác → giữ nguyên |
| **Khác** | **Giống** | **Xung đột thật** — tên bị chiếm → tự thêm hậu tố |
| Khác | Khác | Mới → import |

### Quy tắc hậu tố
Trùng tên nhưng khác ruột → tự thêm `-1`, `-2` (kiểu file tải về, nhưng **gạch ngang không ngoặc** vì alias phải gõ được):

```
cht        ← đã có
cht-1      ← cái mới import vào
```

### Không hỏi gì — chạy một mạch
Import không popup. Xong rồi hiện tổng kết, người dùng tự dọn.

**Nhưng bắt buộc chặn bản sao giống hệt.** Đây là bài học từ 7 dòng Antigravity: import lại cùng một bundle (hành vi bình thường — quên đã import chưa nên làm lại cho chắc) không được đẻ ra `cht-1`, `cht-2`, `cht-3` vô nghĩa.

---

## 10. Trạng thái & bằng chứng

### CLI chưa cài → GIỮ, hiện xám

```
  c          Claude · đăng nhập máy này               [đổi tên]
  cht        Claude · htmustc            ✓ sẵn sàng   [sửa] [xoá]
  co-ht      Codex  · htmustc            ⃠ chưa cài Codex CLI
                                           [ Kiểm tra lại ]
```

- Cấu hình **lưu**, alias **chưa tạo**
- Khi cài CLI xong → app hỏi *"Kích hoạt `co-ht` chứ?"* — một cú bấm
- **Từ ngữ:** *"chưa sẵn sàng"* không phải *"disabled"*. "Disabled" nghe như người dùng đã tắt nó → đi tìm nút bật. "Chưa sẵn sàng — thiếu Codex" → đi cài Codex.
- **Trạng thái tính ra mỗi lần mở app, KHÔNG lưu thành cờ.** Lưu cờ → cài Codex rồi mà vẫn xám vì cờ mắc kẹt.
- Dòng tầng 1 (`c`) chỉ có **một** nút `[đổi tên]` — không sửa, không xoá, không ô key/URL

### Test phải gọi SINH CHỮ thật

**Không** ping reachability. **Không** chỉ check auth.

Đúng endpoint theo từng loại CLI:
- Codex `wire_api = "responses"` → `POST /v1/responses`
- Claude → `/v1/messages`

**Lý do (lỗi #4):** `GET /v1/models` trả `200 OK` chứng minh key hợp lệ — **không** chứng minh gọi được model. Ping sai cửa → dấu tích xanh dối trá.

### Phân loại lỗi — 3 loại, cùng triệu chứng, 3 hành động khác nhau

| Triệu chứng | Nghĩa là | Người dùng làm gì |
|---|---|---|
| `401` | Key sai | Dán key khác |
| `402` | Hết credit | Nạp tiền |
| Timeout / `500` | Provider chưa map model cho endpoint này | Vào Admin của provider |

**Loại thứ ba là thứ tốn 20 phút curl** và là tính năng đáng giá nhất app này làm được.

### Màn hình cuối
```
✓ cht     — đã test, trả lời sau 1.2s
✗ cht-cse — key hợp lệ nhưng provider timeout
⃠ co-ht   — chưa cài Codex CLI

Đã cài 3 profile. Cấu hình mặc định của `claude` và `codex`
không đi theo bundle — thiết lập riêng trên máy này nếu cần.
```
Dòng cuối: không cảnh báo, không màu đỏ. Chỉ là sự thật cần biết.

> Import xong mà không test thì chỉ chuyển **sự không chắc chắn** từ máy này sang máy kia.

---

## 11. Luồng người dùng

### A. Lần đầu, máy trắng
```
Chưa có profile nào.
  ▸ Nhập từ file bundle
  ▸ Tạo mới
```
Preset sẵn **Claude** + **Codex** (đổi alias được). Nhập: tên alias · provider (metadata) · base URL · key · tên biến env · mức dangerous · model mapping · wire_api (Codex).

### B. Máy mới, có bundle
```
Tìm thấy 3 profile: cht · cht-cse · co-ht
⚠ Chưa cài Codex CLI — co-ht sẽ giữ lại dạng chưa kích hoạt
```
→ dán key nếu thiếu → cài → test → đóng.

### C. Import lần hai (đồng bộ)
```
9 profile không đổi.
1 profile khác — cht (key đã thay đổi)  [xem]
```

---

## 12. Ba nguyên tắc

> **Bundle lưu ý định, không lưu file.**
> **"Chưa sẵn sàng" ≠ "bị tắt".**
> **App sở hữu cái tên, không sở hữu cái được gọi.**

---

## 13. Còn để ngỏ

- **Đích lưu bundle** — file trên đĩa hay cơ chế khác (quyết lúc dựng)
- **Danh sách shell hỗ trợ v1** — zsh chắc chắn; bash / fish / PowerShell tuỳ ưu tiên Windows
- **Ngăn xếp công nghệ** — chưa bàn

---

## Phụ lục: trạng thái hiện tại trên máy Mac

Cái app này thay thế:

```
~/.claude/settings.json            mặc định toàn máy (Anthropic auth)
~/.claude/profiles/htmustc.json    overlay, gọi qua --settings
~/.claude/profiles/htcse.json
~/.codex/config.toml               mặc định + provider must1c
~/.codex/ht.config.toml            gọi qua --profile ht
~/.codex/cse.config.toml
~/.codex/keys/{ht,cse}             key plaintext, chmod 600
~/.zshrc                           hàm cp_claude / cuse / cwho / cp_codex + alias
```

| Alias | CLI | Provider | Trạng thái |
|---|---|---|---|
| `c` | Claude | Anthropic auth | ✓ bypass on |
| `cht` | Claude | htmustc | ✓ bypass on |
| `cht-cse` | Claude | htcse | ✓ bypass on |
| `codex` | Codex | key cũ | ✗ hết credit |
| `co-ht` | Codex | htmustc | ✓ yolo on |
| `co-cse` | Codex | htcse | ✗ provider timeout |
