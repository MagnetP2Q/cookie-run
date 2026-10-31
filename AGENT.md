# บันทึกส่งต่องาน — Cookie Run Bot

อัปเดต 2026-11-01 (Asia/Bangkok) • โฟลเดอร์โปรเจกต์: `C:\Users\jadsa\OneDrive\Desktop\cookie-run`

## เริ่มตรงนี้

อ่าน `README.md`, `AGENTS.md` และ `WORK_HISTORY.md` ก่อนทำงาน ตรวจไฟล์และสถานะจริงอีกครั้ง: โฟลเดอร์นี้ **ยังไม่ใช่ Git repository** ณ เวลาบันทึก และโปรเจกต์ Python เดิมอยู่แยกที่ `C:\Users\jadsa\OneDrive\Desktop\Bot_Cookie` อย่าแก้หรือลบข้อมูลเดิมโดยไม่จำเป็น

ผู้ใช้ต้องการโปรแกรม Windows แบบติดตั้ง → ตั้ง ADB path → เลือก emulator → เริ่มบอท UI ต้องกะทัดรัด ใช้ภาพอนิเมะเป็นพื้นหลัง หน้าต่าง 700 × 500 คงที่ ไม่มีขอบ Windows และใช้ปุ่มย่อ/ปิดใน React อย่าเพิ่ม UI ตกแต่งที่ไม่จำเป็น

## สถานะล่าสุด

- React แสดงตัวเลือก emulator พร้อมชื่อ LDPlayer เมื่ออ่านได้, ปุ่มเริ่ม/หยุด, log และหน้าตั้งค่า ADB; ปุ่มรีเฟรชแยกสถานะโหลดจากปุ่มอื่นเพื่อลดการกะพริบ
- Rust เป็นผู้ควบคุม session และลูป ADB → crop → Windows OCR → ตัดสินใจกด; หยุดด้วย cancellation; React ติดต่อผ่าน Tauri commands และ events
- กฎปัจจุบันมีเพียง `box1`, `box2`, `box7`, `box8`, `box9`, `mini`; ห้ามนำกฎซื้อ upgrade/boost เก่ากลับมาเอง ตรวจ default และไฟล์ AppData แยกกันก่อนแก้
- กฎ Play สองตำแหน่ง, Result และ Mystery Box ผ่านลูปบน emulator จริงแล้ว Confirm ผ่าน OCR/ลูปด้วยภาพจำลอง แต่ยังไม่พบหน้าจอจริง; Surprise OCR ผ่านภาพจำลอง ตัวแก้มินิเกมยังไม่ได้ทดสอบจริง
- ตั้งค่า NSIS แล้วและเคย build ติดตั้งบนเครื่องนี้ ทดสอบตั้ง ADB, เลือก emulator และ OCR Result/Mystery Box สำเร็จ; ไม่ใช่เครื่องสะอาด และไม่ได้พิสูจน์การดาวน์โหลดจากบริการออนไลน์
- ไอคอน setup กำหนดแยกด้วย `bundle.windows.nsis.installerIcon` และตรวจไอคอนใน setup ที่ build แล้ว; ไอคอนแอปอยู่ใน `src-tauri/icons/` และฝังใน build; `public/app-icon-concept.png` เป็นต้นฉบับ ส่วน `public/app-icon-32.png` ใช้ใน UI/favicon ภาพพื้นหลังคือ `public/anime-companion.png`
- ล่าสุดเอาข้อความ path ของไฟล์กฎออกจากหน้าตั้งค่าแล้ว source/frontend build ผ่าน ตัวติดตั้งที่เคยสร้างไม่ถือว่าอัปเดตตาม source อัตโนมัติ

## จุดที่ควรทำต่อเมื่อเกี่ยวข้อง

1. ทดสอบตัวติดตั้งบน Windows เครื่องสะอาด: WebView2, English OCR language pack, emulator/ADB path, 1600 × 900 และการเริ่ม–หยุด พร้อมบันทึกผลจริง
2. ตัดสินใจและตรวจแนวทางแจกจ่ายที่รองรับ Windows OCR: Microsoft ระบุ package identity; NSIS ผ่านเครื่องพัฒนานี้แต่ไม่ได้พิสูจน์ทุกเครื่อง ส่วน MSIX script ยังไม่ผ่านการติดตั้ง/เซ็นจริง
3. หากผู้ใช้ต้องการ ค่อยตรวจ Confirm จากหน้าจอจริงและมินิเกมด้วยภาพ/สถานการณ์จริง; อย่าถือผล synthetic เป็นหลักฐานว่ามินิเกมทำงาน
4. Installer หรือ release ที่ build ก่อนการเปลี่ยนแปลงต้อง build ใหม่และติดตั้งซ้ำ; ไม่ควรยืนยันว่าอัปเดตแล้วจากผล frontend build อย่างเดียว

## จุดสำคัญในโค้ด

- `src/app/App.tsx`, `src/style.css`, `src/lib/backend.ts`: UI, สถานะ, command/event
- `src-tauri/src/commands/mod.rs`, `bot/mod.rs`: Tauri commands, session และ scan loop
- `src-tauri/src/adb/mod.rs`: ADB และจับคู่ชื่อ LDPlayer; อุปกรณ์ที่ไม่ตรงรูปแบบ fallback เป็น ID
- `src-tauri/src/config/mod.rs`, `ocr/mod.rs`, `games/cookie_run/mod.rs`: config, OCR และ pHash
- `packaging/build-msix.ps1`: สคริปต์ MSIX ที่ยังต้องทดสอบ; ห้าม commit certificate/password และระวัง password ใน command-line arguments

ดูคำสั่งใน README ก่อนรันทดสอบ เทสต์ live กดเกมจริงได้และอาจใช้ทรัพยากร อย่ารันเพื่อดู UI โดยไม่จำเป็น
