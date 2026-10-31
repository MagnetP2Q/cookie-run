#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    io::Read,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
pub fn run(path: &str, args: &[&str], cancel: &Arc<AtomicBool>) -> Result<Vec<u8>, String> {
    let mut c = Command::new(path);
    c.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    c.creation_flags(0x08000000);
    let mut child = c.spawn().map_err(|e| format!("เริ่ม ADB ไม่ได้: {e}"))?;
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let a = thread::spawn(move || {
        let mut b = Vec::new();
        let _ = out.read_to_end(&mut b);
        b
    });
    let b = thread::spawn(move || {
        let mut v = Vec::new();
        let _ = err.read_to_end(&mut v);
        v
    });
    let start = Instant::now();
    let result = loop {
        if cancel.load(Ordering::SeqCst) || start.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            let _ = child.wait();
            break Err("ADB ถูกยกเลิกหรือหมดเวลา".to_string());
        }
        match child.try_wait() {
            Ok(Some(s)) => {
                break if s.success() {
                    Ok(())
                } else {
                    Err(format!("ADB ล้มเหลว: {s}"))
                }
            }
            Ok(None) => thread::sleep(Duration::from_millis(30)),
            Err(e) => break Err(e.to_string()),
        }
    };
    let output = a.join().unwrap_or_default();
    let stderr = b.join().unwrap_or_default();
    result.map_err(|e| format!("{e} {}", String::from_utf8_lossy(&stderr)))?;
    Ok(output)
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
