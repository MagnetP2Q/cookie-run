# การวัดการใช้ทรัพยากร (Windows)

`measure-soak.ps1` เก็บ CSV ตามช่วงเวลาโดยไม่สั่งให้บอทเริ่มหรือหยุด และไม่เปลี่ยนการตั้งค่าระบบ รันกับ **release build** พร้อม emulator จริง เฉพาะเมื่อผู้ใช้ยินยอมให้บอทเล่นเกม รายการข้อมูลมีเวลา UTC, private bytes, working set, handles, threads และ CPU (`cpu_cores` = CPU seconds ที่ใช้ / เวลาจริง; 1.0 หมายถึงหนึ่ง logical core)

## เตรียม PID

เปิดแอป release แล้วดู PID และ path ให้แน่ใจว่าไม่ได้เลือก development build:

```powershell
Get-CimInstance Win32_Process -Filter "name = 'cookie-run.exe'" | Select-Object ProcessId, ParentProcessId, ExecutablePath
& 'C:\LDPlayer\LDPlayer14\ldconsole.exe' list2
Get-CimInstance Win32_Process -Filter "name = 'adb.exe'" | Select-Object ProcessId, ParentProcessId, ExecutablePath
```

`list2` ของ LDPlayer ให้ PID ของหน้าต่างและ PID ของ emulator/headless แยกกัน ใส่ **ทั้งสอง PID ของ instance ที่ใช้จริง** ใน `-EmulatorProcessIds` เพื่อไม่มองข้าม process หลักของ emulator ตรวจ PID/path อีกครั้งก่อนใช้ หากต้องการวัด ADB server ที่แชร์ ให้ส่ง PID ผ่าน `-AdbServerProcessIds` แต่ห้ามนำยอดนั้นไปเหมาว่าเป็นภาระของบอทเพียงตัวเดียว

## ตัวอย่าง

เปลี่ยน PID ก่อนรัน ควรวัด idle และ active แยกกันด้วยค่า interval, เครื่อง และ build เดียวกัน:

```powershell
# ตัวอย่าง 10 นาที ตัวเลข PID เป็นเพียงตัวอย่าง
powershell -ExecutionPolicy Bypass -File .\tools\measure-soak.ps1 `
  -AppProcessId 1234 -EmulatorProcessIds 2345,3456 -AdbServerProcessIds 4567 `
  -IntervalSeconds 60 -DurationSeconds 600
```

ค่าเริ่มต้นวัดทุก 60 วินาที เป็นเวลา 30 วัน เก็บใต้ `%LOCALAPPDATA%\CookieRunBotMetrics` ในโฟลเดอร์ session ใหม่แต่ละครั้ง; เปลี่ยนได้ด้วย `-OutputRoot` ไฟล์จำกัดโดย `-MaxTotalMiB` (ค่าเริ่มต้น 256 MiB) เมื่อถึงขีดจะหยุดวัดและแจ้ง error โดยไม่ลบข้อมูลเดิม ตรวจพื้นที่ดิสก์และติดตาม process ผู้วัดระหว่างรัน

## วิธีตีความและข้อจำกัด

- `app` คือ process เดิมที่เลือกตาม PID และเวลาเริ่ม `webview2` เป็นเฉพาะ descendants ของแอปตาม process tree; ไม่รวม WebView2 ทั้งเครื่อง และอาจไม่ครบหากมี reparent/shared process ต้องตรวจสอบด้วย WebView2 user-data folder หรือเครื่องมือ Windows เพิ่มเติมก่อนสรุปยอดรวม
- `adb_child` เฉพาะลูกของแอปที่ยังอยู่ ณ เวลาสุ่ม; ADB เป็น process อายุสั้นจึงอาจหายไประหว่าง samples ค่า CPU ที่อ่านได้เป็นเพียง lower-bound และ **ไม่ใช่ CPU รวมทั้งหมด**; ใช้ ETW/process tracing หากต้องการนับทุก subprocess
- `adb_shared` คือ PID ADB ที่ระบุโดยผู้ใช้ ซึ่งอาจทำงานให้แอปหรือ emulator อื่นด้วย `emulator` คือ PID ที่ระบุและ descendants เท่านั้น ตรวจสอบการเปลี่ยน PID เมื่อ restart
- `sample_gap_seconds` ใช้สังเกตช่องว่างระหว่างตัวอย่าง; `app_missing` คือไม่พบ process เดิมหรือ PID ถูกนำไปใช้ใหม่ สคริปต์ไม่เริ่มบอทหรือผูกกับ process ใหม่อัตโนมัติ เมื่อเครื่อง reboot สคริปต์หยุดและไม่มีตัวอย่างช่วงปิดเครื่อง ต้องเริ่ม session ใหม่และบันทึกช่วงขาดเพิ่มเติม
- `completed` หมายถึงตัววัดรันครบเวลา ไม่ได้หมายความว่าบอทเล่นเกมครบจำนวนรอบ ต้องเก็บผลการทำงาน/จำนวนรอบและช่วงขาดแยกต่างหาก และตรวจแนวโน้ม private bytes หลัง warm-up พร้อม CPU, handles และ threads
- ถ้าตัววัดปิดผิดปกติ ไฟล์ไม่มีแถว `completed`; ห้ามตีความเป็นผลทดสอบสำเร็จ และไม่ควรส่ง CSV ออกนอกเครื่องโดยไม่ตรวจข้อมูลก่อน

## วัด fixture soak แบบ opt-in

หากรัน Rust fixture test ที่ build แบบ release สามารถใส่ `-AllowFixtureTestProcess` พร้อม PID ของ test executable ได้ ตัววัดยอมรับเฉพาะ process ชื่อ `cookie_run_lib-<16 hex>` ซึ่งอยู่ใต้ `src-tauri/target/release/deps/` ของโปรเจกต์เท่านั้น แถวหลักใช้กลุ่ม `fixture`; หากไม่ใส่ switch ตัววัดจะปฏิเสธ process นี้ PID ต้องตรงกับ test executable ไม่ใช่ cargo

ผล fixture เป็นการวัด Rust/OCR ที่จำลองการแตะ ไม่รวม UI, WebView2, ADB หรือ emulator และไม่ใช่หลักฐานว่าแอปจริงทำงานครบ 30 วัน
