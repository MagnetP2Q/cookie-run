#[cfg(windows)]
use std::os::windows::{io::AsRawHandle, process::CommandExt};
use std::{
    io::{self, Read},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

// A 1600x900 PNG produced by screencap fits comfortably within this limit,
// even if the image is largely incompressible. Keep diagnostic output bounded.
const MAX_STDOUT: usize = 16 * 1024 * 1024;
const MAX_STDERR: usize = 1024 * 1024;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn PeekNamedPipe(
        pipe: std::os::windows::io::RawHandle,
        buffer: *mut std::ffi::c_void,
        buffer_size: u32,
        bytes_read: *mut u32,
        bytes_available: *mut u32,
        bytes_left: *mut u32,
    ) -> i32;
}

#[cfg(windows)]
fn drain_pipe<R: Read + AsRawHandle>(
    pipe: &mut R,
    buffer: &mut Vec<u8>,
    limit: usize,
    closed: &mut bool,
) -> io::Result<bool> {
    if *closed {
        return Ok(false);
    }
    let mut available = 0u32;
    // The pipe handle is owned and read only by this worker. No other thread
    // performs I/O on it; peeking avoids an indefinitely blocked reader/join.
    let result = unsafe {
        PeekNamedPipe(
            pipe.as_raw_handle(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut available,
            std::ptr::null_mut(),
        )
    };
    if result == 0 {
        let error = io::Error::last_os_error();
        if matches!(error.raw_os_error(), Some(109 | 232)) {
            *closed = true;
            return Ok(false);
        }
        return Err(error);
    }
    if available == 0 {
        return Ok(false);
    }
    if available as usize > limit.saturating_sub(buffer.len()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "command output exceeds limit",
        ));
    }
    let mut chunk = [0u8; 8192];
    let to_read = (available as usize).min(chunk.len());
    let count = pipe.read(&mut chunk[..to_read])?;
    if count == 0 {
        *closed = true;
    } else {
        buffer.extend_from_slice(&chunk[..count]);
    }
    Ok(count != 0)
}

fn kill_and_reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

pub fn run(path: &str, args: &[&str], cancel: &Arc<AtomicBool>) -> Result<Vec<u8>, String> {
    run_with_limits(path, args, cancel, COMMAND_TIMEOUT, MAX_STDOUT, MAX_STDERR)
}

#[cfg(windows)]
fn run_with_limits(
    path: &str,
    args: &[&str],
    cancel: &Arc<AtomicBool>,
    timeout: Duration,
    max_stdout: usize,
    max_stderr: usize,
) -> Result<Vec<u8>, String> {
    if cancel.load(Ordering::SeqCst) {
        return Err("ADB ถูกยกเลิกหรือหมดเวลา".into());
    }
    let mut command = Command::new(path);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000);
    let mut child = command.spawn().map_err(|e| format!("เริ่ม ADB ไม่ได้: {e}"))?;
    let mut out = child.stdout.take().expect("stdout is piped");
    let mut err = child.stderr.take().expect("stderr is piped");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let (mut stdout_closed, mut stderr_closed) = (false, false);
    let mut status = None;
    let started = Instant::now();
    loop {
        if cancel.load(Ordering::SeqCst) || started.elapsed() >= timeout {
            kill_and_reap(&mut child);
            return Err("ADB ถูกยกเลิกหรือหมดเวลา".into());
        }
        let output_progress =
            match drain_pipe(&mut out, &mut stdout, max_stdout, &mut stdout_closed) {
                Ok(progress) => progress,
                Err(error) => {
                    kill_and_reap(&mut child);
                    return Err(format!("อ่าน stdout ของ ADB ไม่ได้: {error}"));
                }
            };
        let error_progress = match drain_pipe(&mut err, &mut stderr, max_stderr, &mut stderr_closed)
        {
            Ok(progress) => progress,
            Err(error) => {
                kill_and_reap(&mut child);
                return Err(format!("อ่าน stderr ของ ADB ไม่ได้: {error}"));
            }
        };
        if status.is_none() {
            match child.try_wait() {
                Ok(value) => status = value,
                Err(error) => {
                    kill_and_reap(&mut child);
                    return Err(format!("ตรวจสถานะ ADB ไม่ได้: {error}"));
                }
            }
        }
        if let Some(exit) = status {
            if stdout_closed && stderr_closed {
                return if exit.success() {
                    Ok(stdout)
                } else {
                    Err(format!(
                        "ADB ล้มเหลว: {exit} {}",
                        String::from_utf8_lossy(&stderr)
                    ))
                };
            }
        }
        if !output_progress && !error_progress {
            thread::sleep(Duration::from_millis(30));
        }
    }
}

#[cfg(not(windows))]
fn run_with_limits(
    _path: &str,
    _args: &[&str],
    _cancel: &Arc<AtomicBool>,
    _timeout: Duration,
    _max_stdout: usize,
    _max_stderr: usize,
) -> Result<Vec<u8>, String> {
    Err("ADB process monitoring is supported only on Windows".into())
}

pub fn devices(path: &str) -> Result<Vec<String>, String> {
    let c = Arc::new(AtomicBool::new(false));
    let out = run(path, &["devices"], &c)?;
    Ok(String::from_utf8_lossy(&out)
        .lines()
        .skip(1)
        .filter_map(|l| {
            let mut p = l.split_whitespace();
            let id = p.next()?;
            if p.next() == Some("device") {
                Some(id.to_string())
            } else {
                None
            }
        })
        .collect())
}
#[derive(Debug, serde::Serialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: Option<String>,
}

// LDPlayer maps instance index 0 to emulator-5554 and increments by two.
// Only use names from the console next to the configured ADB executable.
fn ldplayer_names(output: &str) -> std::collections::HashMap<u32, String> {
    let mut names = std::collections::HashMap::new();
    for line in output.lines() {
        // Split from the right so an instance name containing commas remains intact.
        let fields: Vec<_> = line.rsplitn(9, ',').collect();
        if fields.len() != 9 || fields[5].trim() != "1" {
            continue;
        }
        let Some((index, name)) = fields[8].split_once(',') else {
            continue;
        };
        let Ok(index) = index.trim().parse::<u32>() else {
            continue;
        };
        let name = name.trim();
        if !name.is_empty() {
            names.insert(index, name.to_owned());
        }
    }
    names
}
fn ldplayer_index(id: &str) -> Option<u32> {
    let port = if let Some(number) = id.strip_prefix("emulator-") {
        let port = number.parse::<u32>().ok()?;
        if port < 5554 || (port - 5554) % 2 != 0 {
            return None;
        }
        port - 5554
    } else {
        let number = id
            .strip_prefix("127.0.0.1:")
            .or_else(|| id.strip_prefix("localhost:"))?;
        let port = number.parse::<u32>().ok()?;
        if port < 5555 || (port - 5555) % 2 != 0 {
            return None;
        }
        port - 5555
    };
    Some(port / 2)
}
pub fn devices_with_names(path: &str) -> Result<Vec<DeviceInfo>, String> {
    let ids = devices(path)?;
    let console = std::path::Path::new(path)
        .parent()
        .map(|dir| dir.join("ldconsole.exe"));
    let names = console
        .filter(|p| p.is_file())
        .and_then(|p| {
            let cancel = Arc::new(AtomicBool::new(false));
            let bytes = run(p.to_str()?, &["list2"], &cancel).ok()?;
            String::from_utf8(bytes).ok().map(|s| ldplayer_names(&s))
        })
        .unwrap_or_default();
    Ok(ids
        .into_iter()
        .map(|id| {
            let name = ldplayer_index(&id).and_then(|index| names.get(&index).cloned());
            DeviceInfo { id, name }
        })
        .collect())
}

#[cfg(test)]
mod device_name_tests {
    use super::*;
    #[test]
    fn parses_running_ldplayer_names_and_ports() {
        let names = ldplayer_names("0,Cookie Run,1,2,1,10,11,1600,900,240\n1,LRG,3,4,1,20,21,1280,720,240\n2,Offline,3,4,0,20,21,1280,720,240\n3,Test, name,3,4,1,20,21,1280,720,240\n");
        assert_eq!(
            names
                .get(&ldplayer_index("emulator-5554").unwrap())
                .map(String::as_str),
            Some("Cookie Run")
        );
        assert_eq!(
            names
                .get(&ldplayer_index("emulator-5556").unwrap())
                .map(String::as_str),
            Some("LRG")
        );
        assert_eq!(names.get(&2), None);
        assert_eq!(names.get(&3).map(String::as_str), Some("Test, name"));
        assert_eq!(ldplayer_index("127.0.0.1:5555"), Some(0));
        assert_eq!(ldplayer_index("emulator-5555"), None);
        assert_eq!(ldplayer_index("192.168.1.5:5555"), None);
    }
}

pub fn capture(path: &str, device: &str, c: &Arc<AtomicBool>) -> Result<Vec<u8>, String> {
    run(path, &["-s", device, "exec-out", "screencap", "-p"], c)
}
pub fn tap(path: &str, device: &str, point: [u32; 2], c: &Arc<AtomicBool>) -> Result<(), String> {
    run(
        path,
        &[
            "-s",
            device,
            "shell",
            "input",
            "tap",
            &point[0].to_string(),
            &point[1].to_string(),
        ],
        c,
    )
    .map(|_| ())
}

#[cfg(all(test, windows))]
mod process_tests {
    use super::*;
    fn command(
        script: &str,
        cancel: &Arc<AtomicBool>,
        timeout: Duration,
        out: usize,
        err: usize,
    ) -> Result<Vec<u8>, String> {
        run_with_limits("cmd.exe", &["/D", "/C", script], cancel, timeout, out, err)
    }
    #[test]
    fn returns_stdout_and_reports_process_failure() {
        let cancel = Arc::new(AtomicBool::new(false));
        let output = command("echo hello", &cancel, Duration::from_secs(3), 1024, 1024).unwrap();
        assert_eq!(String::from_utf8_lossy(&output).trim(), "hello");
        let error = command(
            "echo bad 1>&2 & exit /b 3",
            &cancel,
            Duration::from_secs(3),
            1024,
            1024,
        )
        .unwrap_err();
        assert!(error.contains("bad"));
    }
    #[test]
    fn stops_when_stdout_or_stderr_exceeds_limit() {
        let cancel = Arc::new(AtomicBool::new(false));
        let error = command(
            "for /L %i in (1,1,1000) do @echo 1234567890",
            &cancel,
            Duration::from_secs(5),
            128,
            1024,
        )
        .unwrap_err();
        assert!(
            error.contains("stdout") && error.contains("limit"),
            "{error}"
        );
        let error = command(
            "for /L %i in (1,1,1000) do @echo 1234567890 1>&2",
            &cancel,
            Duration::from_secs(5),
            1024,
            128,
        )
        .unwrap_err();
        assert!(
            error.contains("stderr") && error.contains("limit"),
            "{error}"
        );
    }
    #[test]
    fn cancellation_kills_running_child_promptly() {
        let cancel = Arc::new(AtomicBool::new(false));
        let signal = cancel.clone();
        let thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            signal.store(true, Ordering::SeqCst)
        });
        let start = Instant::now();
        let error = run_with_limits(
            "powershell.exe",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 5",
            ],
            &cancel,
            Duration::from_secs(10),
            1024,
            1024,
        )
        .unwrap_err();
        thread.join().unwrap();
        assert!(error.contains("ยกเลิก"));
        assert!(start.elapsed() < Duration::from_secs(3));
    }
    #[test]
    fn timeout_does_not_hang_if_descendant_keeps_pipe_open() {
        let cancel = Arc::new(AtomicBool::new(false));
        let start = Instant::now();
        // start /b inherits the output handles but exits on its own after ~2 seconds.
        let result = command(
            "start \"\" /b ping -n 3 127.0.0.1 & exit /b 0",
            &cancel,
            Duration::from_millis(400),
            8192,
            8192,
        );
        let error = result.expect_err("expected timeout when descendant holds the pipe");
        assert!(error.contains("หมดเวลา"), "unexpected error: {error}");
        assert!(start.elapsed() < Duration::from_secs(3));
    }
    #[test]
    fn timeout_kills_hung_child() {
        let cancel = Arc::new(AtomicBool::new(false));
        let start = Instant::now();
        let result = run_with_limits(
            "powershell.exe",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 5",
            ],
            &cancel,
            Duration::from_millis(200),
            1024,
            1024,
        );
        assert!(result.unwrap_err().contains("หมดเวลา"));
        assert!(start.elapsed() < Duration::from_secs(3));
    }
}

#[cfg(all(test, windows))]
mod live_adb_tests {
    use super::*;
    #[test]
    #[ignore = "requires an ADB executable and running emulator; capture only"]
    fn lists_devices_and_captures_png() {
        let path = std::env::var("COOKIE_RUN_TEST_ADB").expect("set COOKIE_RUN_TEST_ADB");
        let id = std::env::var("COOKIE_RUN_TEST_DEVICE").expect("set COOKIE_RUN_TEST_DEVICE");
        let connected = devices(&path).unwrap();
        assert!(connected.contains(&id));
        let bytes = capture(&path, &id, &Arc::new(AtomicBool::new(false))).unwrap();
        let img = image::load_from_memory(&bytes).unwrap();
        println!(
            "capture {} bytes, {}x{}",
            bytes.len(),
            img.width(),
            img.height()
        );
        assert_eq!((img.width(), img.height()), (1600, 900));
    }
}
