# บันทึกส่งต่องาน — Cookie Run Bot

อัปเดต 2026-11-01 (Asia/Bangkok) • โฟลเดอร์โปรเจกต์: `C:\Users\jadsa\OneDrive\Desktop\cookie-run`

## เริ่มตรงนี้

อ่าน `README.md`, `AGENTS.md` และ `WORK_HISTORY.md` ก่อนทำงาน ตรวจไฟล์และสถานะจริงอีกครั้ง: ณ เวลาบันทึกโฟลเดอร์นี้เป็น Git repository อยู่บน branch `optimization` ที่ commit `a8c0669` และมีงานระบบ log กับเอกสารที่ยังไม่ได้ commit (รวมไฟล์ใหม่ `src-tauri/src/logging.rs`); โปรเจกต์ Python เดิมอยู่แยกที่ `C:\Users\jadsa\OneDrive\Desktop\Bot_Cookie` อย่าแก้หรือลบข้อมูลเดิมโดยไม่จำเป็น

ผู้ใช้ต้องการโปรแกรม Windows แบบติดตั้ง → ตั้ง ADB path → เลือก emulator → เริ่มบอท UI ต้องกะทัดรัด ใช้ภาพอนิเมะเป็นพื้นหลัง หน้าต่าง 700 × 500 คงที่ ไม่มีขอบ Windows และใช้ปุ่มย่อ/ปิดใน React อย่าเพิ่ม UI ตกแต่งที่ไม่จำเป็น

## สถานะล่าสุด

- React แสดงตัวเลือก emulator พร้อมชื่อ LDPlayer เมื่ออ่านได้, ปุ่มเริ่ม/หยุด, log และหน้าตั้งค่า ADB; ปุ่มรีเฟรชแยกสถานะโหลดจากปุ่มอื่นเพื่อลดการกะพริบ
- Rust เป็นผู้ควบคุม session และลูป ADB → crop → Windows OCR → ตัดสินใจกด; หยุดด้วย cancellation; React ติดต่อผ่าน Tauri commands และ events
- กฎปัจจุบันมีเพียง `box1`, `box2`, `box7`, `box8`, `box9`, `mini`; ห้ามนำกฎซื้อ upgrade/boost เก่ากลับมาเอง ตรวจ default และไฟล์ AppData แยกกันก่อนแก้
- กฎ Play สองตำแหน่ง, Result และ Mystery Box ผ่านลูปบน emulator จริงแล้ว Confirm ผ่าน OCR/ลูปด้วยภาพจำลอง แต่ยังไม่พบหน้าจอจริง; Surprise OCR ผ่านภาพจำลอง ตัวแก้มินิเกมยังไม่ได้ทดสอบจริง
- ตั้งค่า NSIS แล้วและเคย build ติดตั้งบนเครื่องนี้ ทดสอบตั้ง ADB, เลือก emulator และ OCR Result/Mystery Box สำเร็จ; ไม่ใช่เครื่องสะอาด และไม่ได้พิสูจน์การดาวน์โหลดจากบริการออนไลน์
- ไอคอน setup กำหนดแยกด้วย `bundle.windows.nsis.installerIcon` และตรวจไอคอนใน setup ที่ build แล้ว; ไอคอนแอปอยู่ใน `src-tauri/icons/` และฝังใน build; `public/app-icon-concept.png` เป็นต้นฉบับ ส่วน `public/app-icon-32.png` ใช้ใน UI/favicon ภาพพื้นหลังคือ `public/anime-companion.png`
- Activity log แสดงเฉพาะ `วนลูปครั้งที่ N` พร้อมเวลา ไม่เกิน 100 รายการล่าสุด นับหลัง `box2` ส่งคำสั่งแตะ ADB สำเร็จ และเริ่มที่ 1 ใหม่ทุกครั้งที่เริ่มบอท การล้าง log ไม่รีเซ็ตเลขรอบใน session ปัจจุบัน
- `src-tauri/src/logging.rs` ใช้ worker เรียงลำดับเขียน/อ่าน/ล้าง บันทึก OCR แต่ละ scan และเหตุการณ์ใน `%APPDATA%\com.cookie.run\cookie.log` เป็น JSON Lines หมุนไฟล์เมื่อจะเกิน 5 MiB เก็บ `.1` และ `.2` ล้างทั้งสามไฟล์ และแจ้ง error ใน UI เมื่อเขียนไม่ได้; UI โหลดประวัติเมื่อเปิดแอป
- ระบบ log พักเป็นชุด 60 วินาทีหลังรายการแรก; error, หยุดบอท และปิดแอปตามปกติเร่งเขียน คิวจำกัด 2,000 รายการหรือ 1 MiB รวมรายการที่กำลังเขียน เมื่อเต็มทิ้ง OCR เก่าก่อนและแจ้งเตือน; การล้างรวม buffer ด้วย
- ระบบ log ยังไม่ได้ตรวจ end-to-end บนแอป Tauri/เกมจริง รวมทั้งการล้างระหว่างรัน การปิดหน้าต่าง และยังไม่มีผลทดสอบโหลด/ใช้ RAM ระยะยาวหลังเพิ่ม log
- ตัวติดตั้ง NSIS ที่มีอยู่สร้างก่อนการเปลี่ยนระบบ log นี้ และแอปที่ติดตั้งอยู่ไม่อัปเดตเอง ต้อง build และติดตั้งใหม่ก่อนทดสอบรุ่นล่าสุด

## สถานะการปรับ CPU/RAM ล่าสุด

มีการใช้ OCR engine ซ้ำต่อ worker, ปิดทรัพยากร WinRT, ลดการถือภาพเต็ม และจัดการ ADB subprocess/stop/panic แล้ว baseline สั้นอยู่ใน `measurements/` (ไม่เก็บใน Git) `tools/METRICS.md` อธิบายเครื่องมือวัดและข้อจำกัด Rust release tests และ OCR fixtures ผ่าน แต่ยังไม่ผ่านการรัน 24–72 ชั่วโมงหรือ 30 วัน ผู้ใช้จะทดลองใช้งานเอง 1–3 วันและแจ้งผล จึงอย่าเริ่มการทดสอบยาว/กดเกมเองโดยพลการ NSIS ที่มีอยู่มีการปรับ CPU/RAM แต่ไม่รวมระบบ log ล่าสุด; ต้อง build ใหม่หากต้องการใช้ทั้งสองส่วน

## จุดที่ควรทำต่อเมื่อเกี่ยวข้อง

0. ตรวจระบบ log ในแอปจริงเมื่อเหมาะสม: การนับเฉพาะหลังแตะ `box2`, เริ่ม session ใหม่, เปิดแอปโหลดประวัติ, ล้างขณะรัน, หมุนไฟล์และแจ้งข้อผิดพลาด โดยอย่าสั่งแตะเกมเองหากยังไม่มีการอนุญาต; build NSIS ใหม่ก่อนหากต้องการตัวติดตั้ง
1. รอผู้ใช้ทดลองรุ่นปรับปรุงเอง 1–3 วันแล้วสอบถาม/วิเคราะห์ RAM, CPU, อาการค้างหรือ error จากข้อมูลที่มี อย่ารันเกมหรือ soak test ยาวแทนผู้ใช้โดยพลการ และอย่าสรุปผล 30 วันจากการทดลองระยะสั้น
2. ทดสอบตัวติดตั้งบน Windows เครื่องสะอาด: WebView2, English OCR language pack, emulator/ADB path, 1600 × 900 และการเริ่ม–หยุด พร้อมบันทึกผลจริง
3. ตัดสินใจและตรวจแนวทางแจกจ่ายที่รองรับ Windows OCR: Microsoft ระบุ package identity; NSIS ผ่านเครื่องพัฒนานี้แต่ไม่ได้พิสูจน์ทุกเครื่อง ส่วน MSIX script ยังไม่ผ่านการติดตั้ง/เซ็นจริง
4. หากผู้ใช้ต้องการ ค่อยตรวจ Confirm จากหน้าจอจริงและมินิเกมด้วยภาพ/สถานการณ์จริง; อย่าถือผล synthetic เป็นหลักฐานว่ามินิเกมทำงาน
5. Installer หรือ release ที่ build ก่อนการเปลี่ยนแปลงต้อง build ใหม่และติดตั้งซ้ำ; ไม่ควรยืนยันว่าอัปเดตแล้วจากผล frontend build อย่างเดียว

## จุดสำคัญในโค้ด

- `src/app/App.tsx`, `src/style.css`, `src/lib/backend.ts`: UI, สถานะ, command/event
- `src-tauri/src/commands/mod.rs`, `bot/mod.rs`, `logging.rs`: Tauri commands, session, scan loop และระบบเก็บ/หมุน log
- `src-tauri/src/adb/mod.rs`: ADB และจับคู่ชื่อ LDPlayer; อุปกรณ์ที่ไม่ตรงรูปแบบ fallback เป็น ID
- `src-tauri/src/config/mod.rs`, `ocr/mod.rs`, `games/cookie_run/mod.rs`: config, OCR และ pHash
- `packaging/build-msix.ps1`: สคริปต์ MSIX ที่ยังต้องทดสอบ; ห้าม commit certificate/password และระวัง password ใน command-line arguments

ดูคำสั่งใน README ก่อนรันทดสอบ เทสต์ live กดเกมจริงได้และอาจใช้ทรัพยากร อย่ารันเพื่อดู UI โดยไม่จำเป็น
