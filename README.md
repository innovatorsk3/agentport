# agentport

**Mang cấu hình Claude Code và Codex CLI sang máy khác — kể cả khác hệ điều hành. Export, import, test, xong.**

> ⚠️ Đang phát triển. Chưa có bản chạy được.

---

## Vấn đề

Bạn đã dựng xong Claude Code và Codex CLI trên một máy: nhiều provider, alias riêng cho từng cái, bypass permission bật sẵn để khỏi gõ `--dangerously-skip-permissions` mỗi lần.

Giờ bạn ngồi trước một máy khác. Có khi còn khác hệ điều hành.

Làm lại từ đầu mất cả buổi — và những lỗi trên đường đều **im lặng**: config đặt sai chỗ vẫn không báo gì, key nạp sai biến vẫn chạy, provider trả `200 OK` ở endpoint này nhưng treo ở endpoint kia.

## Cách giải

Một app desktop chạy **hai lần trong đời**: một lần export ở máy cũ, một lần import ở máy mới. Ghi config xuống máy rồi đóng. Terminal chạy độc lập sau đó — **kể cả khi bạn gỡ app**.

## Khác gì mấy công cụ đổi provider khác

Đã có vài công cụ đổi provider cho Claude Code. agentport không cạnh tranh ở đó:

| | Công cụ đổi provider | agentport |
|---|---|---|
| Đổi provider trên một máy | ✓ | — |
| **Mang sang máy khác, khác OS** | ✗ | ✓ |
| **Test gọi sinh chữ thật rồi phân loại lỗi** | ✗ | ✓ |

Cái thứ hai đáng giá hơn vẻ ngoài của nó. Ba lỗi này cho ra cùng một triệu chứng "không chạy" nhưng cần ba hành động hoàn toàn khác nhau:

| Trả về | Nghĩa là | Bạn phải làm |
|---|---|---|
| `401` | Key sai | Dán key khác |
| `402` | Hết credit | Nạp tiền |
| Timeout / `500` | Provider chưa map model cho endpoint này | Vào Admin của provider |

Loại thứ ba tốn 20 phút gõ `curl` để tìm ra. agentport nói cho bạn trong một dòng.

## Nguyên tắc

**Không sở hữu file khởi động shell.** File `.zshrc` (hoặc `$PROFILE` trên Windows) chỉ nhận đúng **một dòng, một lần**. Gỡ app = xoá một dòng.

**Bundle lưu ý định, không lưu file.** Không đường dẫn tuyệt đối, không JSON/TOML nguyên si của CLI. Máy đích tự dịch sang schema của phiên bản CLI đang cài — đó là cách macOS → Windows chạy được.

**App sở hữu cái tên, không sở hữu cái được gọi.** Nó tạo alias, không sửa cấu hình mặc định của CLI hay auth của bạn.

## ⚠️ Bundle chứa API key dạng plaintext

Đây là **chủ ý** — công cụ dùng nội bộ, key phải đi theo được và đọc được.

Nghĩa là: **đừng bao giờ commit file bundle.** `.gitignore` đã chặn sẵn `*.agentport` và các biến thể, nhưng hãy tự cẩn thận thêm.

## Tài liệu

- [Requirements](docs/requirements.md) — đặc tả đầy đủ, kèm bằng chứng cho từng quyết định
- [Tech stack & build](docs/techstack.md) — Tauri v2 + React, CI, ký tên

## Giấy phép

Chưa quyết.
