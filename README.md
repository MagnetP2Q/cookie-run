# Cookie Run Bot

บอท Cookie Run บน Windows สร้างด้วย Tauri 2 + Rust และ React/TypeScript ใช้ ADB จับภาพจาก emulator, Windows OCR ตรวจข้อความ และส่งคำสั่งแตะตามกฎ

## ใช้งาน

1. ติดตั้งและเปิด Android emulator พร้อมเกม Cookie Run ตั้งความละเอียด **1600 × 900**
2. เปิดแอป กดปุ่มตั้งค่า และระบุ path ของ `adb.exe` ค่าเริ่มต้นคือ `C:\LDPlayer\LDPlayer14\adb.exe`
3. กดปุ่มค้นหา เลือก emulator แล้วกด **เริ่มบอท**; กด **หยุด** เพื่อหยุด

เครื่องผู้ใช้ต้องมี ADB, WebView2 Runtime และ Windows OCR language pack ที่อ่านภาษาในเกมได้ แอปไม่ได้รวม emulator หรือ ADB มาให้ เครื่องผู้ใช้ไม่ต้องติดตั้ง Rust, Node.js หรือ Python เพื่อรันโปรแกรมที่ build แล้ว

หน้าต่างมีขนาดคงที่ 700 × 500 และใช้ title bar ที่สร้างเอง ภาพพื้นหลังและไอคอนอยู่ใน `public/` เมื่อใช้ ADB ที่อยู่คู่กับ `ldconsole.exe` แอปพยายามแสดงชื่อ LDPlayer ต่อท้าย ID หากอ่านชื่อไม่ได้จะแสดง ID อย่างเดียว การสั่งบอทใช้ ID เสมอ

## Activity log

หน้าจอแสดงเฉพาะรอบล่าสุดไม่เกิน 100 รายการ พร้อมเวลาและรายการใหม่อยู่ด้านบน นับรอบเมื่อกฎ `box2` ส่งคำสั่งแตะ ADB สำเร็จเท่านั้น และเริ่มนับใหม่ทุกครั้งที่เริ่มบอท ประวัติ session ก่อนยังอยู่จนกว่าจะถูกล้างหรือถูกหมุนทิ้ง

รายละเอียด OCR แต่ละการสแกน เหตุการณ์ที่พบ สถานะเริ่ม/หยุด ข้อผิดพลาด และรอบ ถูกบันทึกเป็น JSON Lines ใน `%APPDATA%\com.cookie.run\cookie.log` ระบบหมุนไฟล์เมื่อจะเกินประมาณ 5 MiB เก็บไฟล์เก่าสูงสุดสองไฟล์ (`cookie.log.1`, `cookie.log.2`) รวมประมาณ 15 MiB เมื่อกดล้าง log จะลบทั้งสามไฟล์และล้างหน้าจอ โดยไม่รีเซ็ตเลขรอบของ session ที่กำลังทำงาน หากเขียนไม่สำเร็จแอปจะแสดงข้อผิดพลาด

Log จะพักในหน่วยความจำและเขียนเป็นชุด 60 วินาทีหลังรายการแรก โดยเขียนเร็วขึ้นเมื่อหยุดบอท ปิดแอปตามปกติ หรือเกิด error คิวจำกัด 2,000 รายการหรือประมาณ 1 MiB หากเต็มจะทิ้งรายการ OCR เก่าก่อนและแจ้งเตือน การล้าง log ล้างรายการที่รอเขียนด้วย หากแอปหรือเครื่องดับกะทันหัน รายการที่ยังรอเขียนอาจหายได้ หากเขียนไม่สำเร็จขณะปิด แอปจะยังเปิดอยู่และแจ้งข้อผิดพลาดเพื่อให้ลองใหม่

โค้ด log รุ่นนี้ยังไม่ได้ build ตัวติดตั้งใหม่หรือทดสอบ flow ทั้งหมดในแอป/เกมจริง

## ตั้งค่าสำหรับพัฒนา

ต้องมี Windows, Node.js, Bun, Rust และ Tauri Windows prerequisites (C++ build tools และ WebView2) โปรเจกต์มี `bun.lock` และ Tauri เรียก `bun` ในขั้น dev/build:

```powershell
bun install
npm run tauri dev
```

`npm run build` ตรวจ TypeScript และ build frontend โดยไม่สร้างตัวติดตั้ง ส่วน `npm run dev` เปิดเฉพาะ Vite ไม่ใช่แอป Tauri เต็มรูปแบบ

## กฎและข้อมูลผู้ใช้

- กฎเริ่มต้นอยู่ใน `config/cookie-run.default.json` มี Play สองตำแหน่ง, Result, Mystery Box, Confirm และ Surprise/minigame ตามลำดับ
- การตั้งค่าและกฎที่ใช้งานจริงสร้างใน `%APPDATA%\com.cookie.run\settings.json` และ `%APPDATA%\com.cookie.run\cookie-run.json` โดยสร้างกฎเริ่มต้นเฉพาะเมื่อไฟล์ยังไม่มี การแก้ default **ไม่เขียนทับ** ไฟล์ที่ผู้ใช้มีอยู่
- `interval_ms`, `post_tap_ms`, `scale`, `rect`, `action`, `states` และ `wait_ms` ใช้กำหนดการสแกนและการกระทำ แก้ค่าด้วยความระมัดระวังและตรวจพิกัดกับหน้าจอจริง
- ภาพหน้าจอต้องเป็น 1600 × 900; หากไม่ตรงบอทจะแสดงข้อผิดพลาด กฎ Surprise เปิดใช้งานอยู่ แต่ตัวแก้มินิเกมยังไม่ได้ยืนยันกับเกมจริง

## ทดสอบ

```powershell
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --lib
# Windows OCR + ภาพจำลอง ไม่แตะ emulator
cargo test --manifest-path src-tauri/Cargo.toml --lib every_configured_region_is_recognized -- --ignored --nocapture
cargo test --manifest-path src-tauri/Cargo.toml --lib full_loop_with_real_ocr_and_simulated_taps -- --ignored --nocapture
```

`run_live_without_minigame` เป็น ignored test ที่สั่งแตะเกมจริงและอาจใช้ทรัพยากรในเกม ตรวจ device/path ที่ hardcode ในเทสต์ก่อนรัน และรันเฉพาะเมื่อมีการอนุญาตให้ทดสอบบน emulator จริง

## การใช้ทรัพยากรและการทดลองระยะยาว

รุ่นล่าสุดปรับให้ OCR engine ใช้ซ้ำต่อ worker ปิดทรัพยากร WinRT หลังใช้งาน จำกัดข้อมูลจาก ADB และจัดการการหยุด/ข้อผิดพลาดของ worker โดยยังตรวจแต่ละกฎด้วยภาพใหม่ตามลำดับเดิม ดูรายละเอียดการวัดและข้อจำกัดที่ [tools/METRICS.md](tools/METRICS.md)

เก็บ baseline แบบสั้นแยก idle และ active ไว้ใน `measurements/` ซึ่งไม่รวมใน Git ค่าเหล่านี้ไม่ใช่หลักฐานยืนยันการทำงานต่อเนื่อง 24 ชั่วโมงหรือ 30 วัน ผู้ใช้จะทดลองรุ่นปรับปรุงเองประมาณ 1–3 วันแล้วแจ้งผล; ยังไม่ควรสรุปว่า RAM/CPU คงที่ระยะยาว ตัวติดตั้งรุ่นปรับ CPU/RAM เคย build แล้ว แต่ยังไม่ได้ยืนยันว่าติดตั้งทับรุ่นเดิม และยังไม่มีการ build ตัวติดตั้งที่รวมระบบ log ล่าสุด

## สร้างตัวติดตั้ง Windows

```powershell
npm run tauri -- build --bundles nsis
```

ผลลัพธ์อยู่ใน `src-tauri/target/release/bundle/nsis/` ตัวติดตั้งที่สร้างเคยติดตั้งและทดสอบบนเครื่องพัฒนานี้ได้ รวมถึงตั้ง ADB, ค้นหา LDPlayer, เริ่มบอท และ OCR พบ Result/Mystery Box แต่เครื่องนี้มี dependency และการตั้งค่าเดิมอยู่แล้ว จึงยังไม่ใช่การยืนยันบนเครื่องสะอาด ต้อง build/ติดตั้งใหม่หลังแก้ source; ตัวที่ติดตั้งอยู่ไม่อัปเดตเอง

ตัวติดตั้ง Tauri แบบปกติอาจดาวน์โหลด WebView2 หากเครื่องยังไม่มี ส่วน `packaging/build-msix.ps1` เป็นแนวทางแพ็ก MSIX ที่ต้องใช้ signing certificate ซึ่งเครื่องปลายทางเชื่อถือ สคริปต์และการติดตั้ง MSIX ยังไม่ได้ทดสอบครบ Microsoft ระบุว่า Windows.Media.Ocr สำหรับ desktop รองรับแอปที่มี package identity แม้ NSIS จะทำงานบนเครื่องนี้แล้วก็ตาม จึงต้องตรวจรูปแบบแจกจ่ายและทดสอบเครื่อง Windows สะอาดก่อนรับประกันการใช้งาน

ดูรายละเอียดการส่งต่องานใน [AGENT.md](AGENT.md) และประวัติใน [WORK_HISTORY.md](WORK_HISTORY.md)
