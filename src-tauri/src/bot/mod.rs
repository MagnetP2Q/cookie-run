use crate::{
    adb,
    config::{Action, Config},
    games::cookie_run,
    ocr::{Ocr, WindowsOcr},
};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};
#[derive(Clone, Serialize)]
pub struct Status {
    pub state: String,
    pub device: Option<String>,
    pub message: Option<String>,
}
#[derive(Clone, Serialize)]
struct Log {
    level: String,
    message: String,
    timestamp: u64,
}
pub struct Controller {
    pub inner: Mutex<Inner>,
}
pub struct Inner {
    pub status: Status,
    pub cancel: Option<Arc<AtomicBool>>,
}
impl Default for Controller {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner {
                status: Status {
                    state: "stopped".into(),
                    device: None,
                    message: None,
                },
                cancel: None,
            }),
        }
    }
}
fn status(app: &AppHandle, state: &str, device: Option<String>, message: Option<String>) {
    let ctl = app.state::<Controller>();
    let mut i = ctl.inner.lock().unwrap();
    i.status = Status {
        state: state.into(),
        device,
        message,
    };
    let _ = app.emit("bot-status", i.status.clone());
}
fn log(app: &AppHandle, level: &str, message: String) {
    let _ = app.emit(
        "bot-log",
        Log {
            level: level.into(),
            message,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        },
    );
}
fn pause(c: &AtomicBool, ms: u64) -> bool {
    let until = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < until {
        if c.load(Ordering::SeqCst) {
            return false;
        }
        thread::sleep(
            Duration::from_millis(25).min(until.saturating_duration_since(Instant::now())),
        )
    }
    !c.load(Ordering::SeqCst)
}
pub fn start(app: AppHandle, device: String, path: String, cfg: Config, cancel: Arc<AtomicBool>) {
    thread::spawn(move || {
        {
            let ctl = app.state::<Controller>();
            let mut i = ctl.inner.lock().unwrap();
            if cancel.load(Ordering::SeqCst) {
                i.status = Status {
                    state: "stopped".into(),
                    device: None,
                    message: None,
                };
                let _ = app.emit("bot-status", i.status.clone());
                return;
            }
            i.status = Status {
                state: "running".into(),
                device: Some(device.clone()),
                message: None,
            };
            let _ = app.emit("bot-status", i.status.clone());
        }
        log(&app, "INFO", format!("เริ่มทำงานบน {device}"));
        let result = work(&app, &device, &path, cfg, &cancel);
        if cancel.load(Ordering::SeqCst) {
            status(&app, "stopped", None, None);
            log(&app, "INFO", "หยุดบอทแล้ว".into())
        } else if let Err(e) = result {
            log(&app, "ERROR", e.clone());
            status(&app, "error", Some(device), Some(e))
        } else {
            status(&app, "stopped", None, None)
        }
    });
}
trait BotIo {
    fn scan(
        &mut self,
        rule: &crate::config::Rule,
        cfg: &Config,
        cancel: &Arc<AtomicBool>,
    ) -> Result<bool, String>;
    fn act(&mut self, rule: &crate::config::Rule, cancel: &Arc<AtomicBool>) -> Result<(), String>;
    fn found(&mut self, rule: &crate::config::Rule);
}

struct RealIo<'a> {
    app: Option<&'a AppHandle>,
    device: &'a str,
    path: &'a str,
    image: Option<image::DynamicImage>,
    text: String,
    ocr: WindowsOcr,
}
impl BotIo for RealIo<'_> {
    fn scan(
        &mut self,
        rule: &crate::config::Rule,
        cfg: &Config,
        cancel: &Arc<AtomicBool>,
    ) -> Result<bool, String> {
        let bytes = adb::capture(self.path, self.device, cancel)?;
        let img = image::load_from_memory(&bytes).map_err(|e| format!("ภาพหน้าจอไม่ถูกต้อง: {e}"))?;
        if img.width() != 1600 || img.height() != 900 {
            return Err(format!(
                "ต้องใช้ความละเอียด 1600x900; พบ {}x{}",
                img.width(),
                img.height()
            ));
        }
        let [l, t, r, b] = rule.rect;
        let crop = img.crop_imm(l, t, r - l, b - t).resize_exact(
            (r - l) * cfg.scale,
            (b - t) * cfg.scale,
            image::imageops::FilterType::Lanczos3,
        );
        self.text = self.ocr.recognize(&crop)?;
        let matched = cookie_run::matches(&rule.label, &self.text);
        self.image = Some(img);
        Ok(matched)
    }
    fn found(&mut self, rule: &crate::config::Rule) {
        if let Some(app) = self.app {
            log(app, "INFO", format!("พบ {}: {}", rule.label, self.text));
        }
    }
    fn act(&mut self, rule: &crate::config::Rule, cancel: &Arc<AtomicBool>) -> Result<(), String> {
        match rule.action {
            Action::Tap { point } => adb::tap(self.path, self.device, point, cancel)?,
            Action::Minigame { region } => {
                let image = self.image.as_ref().ok_or("ไม่พบภาพมินิเกม")?;
                for point in cookie_run::odd_cards(image, region) {
                    if cancel.load(Ordering::SeqCst) {
                        return Ok(());
                    }
                    adb::tap(self.path, self.device, point, cancel)?;
                    if !pause(cancel, 500) {
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}
fn work(
    app: &AppHandle,
    device: &str,
    path: &str,
    cfg: Config,
    cancel: &Arc<AtomicBool>,
) -> Result<(), String> {
    run_loop(
        &cfg,
        cancel,
        &mut RealIo {
            app: Some(app),
            device,
            path,
            image: None,
            text: String::new(),
            ocr: WindowsOcr,
        },
    )
}
fn run_loop(cfg: &Config, cancel: &Arc<AtomicBool>, io: &mut impl BotIo) -> Result<(), String> {
    let mut active: HashMap<String, bool> =
        cfg.rules.iter().map(|r| (r.id.clone(), true)).collect();
    while !cancel.load(Ordering::SeqCst) {
        let mut matched = false;
        for rule in &cfg.rules {
            if cancel.load(Ordering::SeqCst) {
                return Ok(());
            }
            if !active.get(&rule.id).copied().unwrap_or(true) {
                continue;
            }
            let start = Instant::now();
            loop {
                if io.scan(rule, cfg, cancel)? {
                    if cancel.load(Ordering::SeqCst) {
                        return Ok(());
                    }
                    io.found(rule);
                    io.act(rule, cancel)?;
                    for (id, value) in &rule.states {
                        active.insert(id.clone(), *value);
                    }
                    matched = true;
                    if !pause(cancel, cfg.post_tap_ms) {
                        return Ok(());
                    }
                    break;
                }
                if rule.wait_ms == 0 || start.elapsed() >= Duration::from_millis(rule.wait_ms) {
                    break;
                }
                if !pause(cancel, cfg.interval_ms) {
                    return Ok(());
                }
            }
            if matched {
                break;
            }
        }
        if !matched && !pause(cancel, cfg.interval_ms) {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Rule;
    use std::collections::VecDeque;

    fn defaults() -> Config {
        let mut cfg: Config =
            serde_json::from_str(include_str!("../../../config/cookie-run.default.json")).unwrap();
        cfg.interval_ms = 1;
        cfg.post_tap_ms = 0;
        for r in &mut cfg.rules {
            r.wait_ms = 0;
        }
        cfg
    }
    struct Fake {
        targets: VecDeque<&'static str>,
        scans: Vec<String>,
        actions: Vec<(String, [u32; 2])>,
        cancel: Arc<AtomicBool>,
        fail: bool,
    }
    impl BotIo for Fake {
        fn scan(&mut self, rule: &Rule, _: &Config, _: &Arc<AtomicBool>) -> Result<bool, String> {
            self.scans.push(rule.id.clone());
            if self.scans.len() > 300 {
                return Err("unexpected endless scanning".into());
            }
            if self.fail {
                return Err("capture failed".into());
            }
            Ok(self
                .targets
                .front()
                .is_some_and(|target| *target == rule.id))
        }
        fn found(&mut self, _: &Rule) {}
        fn act(&mut self, rule: &Rule, _: &Arc<AtomicBool>) -> Result<(), String> {
            if let Action::Tap { point } = rule.action {
                self.actions.push((rule.id.clone(), point));
            } else {
                return Err("minigame must not run in this test".into());
            }
            self.targets.pop_front();
            if self.targets.is_empty() {
                self.cancel.store(true, Ordering::SeqCst);
            }
            Ok(())
        }
    }
    fn fake(targets: &[&'static str]) -> Fake {
        Fake {
            targets: targets.iter().copied().collect(),
            scans: vec![],
            actions: vec![],
            cancel: Arc::new(AtomicBool::new(false)),
            fail: false,
        }
    }
    #[test]
    fn all_five_tap_rules_work_across_repeated_cycles() {
        let cfg = defaults();
        let mut io = fake(&["box1", "box2", "box7", "box8", "box9"]);
        run_loop(&cfg, &io.cancel.clone(), &mut io).unwrap();
        assert_eq!(
            io.actions,
            vec![
                ("box1".into(), [1195, 807]),
                ("box2".into(), [1123, 764]),
                ("box7".into(), [580, 772]),
                ("box8".into(), [796, 804]),
                ("box9".into(), [793, 794])
            ]
        );
        assert!(io.scans.len() > cfg.rules.len());
    }
    #[test]
    fn disable_and_reenable_rules_in_the_live_loop() {
        let mut cfg = defaults();
        cfg.rules
            .iter_mut()
            .find(|r| r.id == "box7")
            .unwrap()
            .states
            .insert("box1".into(), false);
        cfg.rules
            .iter_mut()
            .find(|r| r.id == "box8")
            .unwrap()
            .states
            .insert("box1".into(), true);
        let mut io = fake(&["box7", "box8", "box1"]);
        run_loop(&cfg, &io.cancel.clone(), &mut io).unwrap();
        let disable = io.scans.iter().position(|s| s == "box7").unwrap();
        let enable = io.scans.iter().position(|s| s == "box8").unwrap();
        assert!(!io.scans[disable + 1..enable].iter().any(|s| s == "box1"));
        assert_eq!(
            io.actions.iter().map(|a| a.0.as_str()).collect::<Vec<_>>(),
            vec!["box7", "box8", "box1"]
        );
    }
    #[test]
    fn retry_wait_and_cancel_stop_further_scans() {
        let mut cfg = defaults();
        cfg.rules.truncate(1);
        cfg.rules[0].wait_ms = 30;
        cfg.interval_ms = 5;
        let mut io = fake(&["box1"]);
        io.targets.clear();
        let cancel = io.cancel.clone();
        let signal = cancel.clone();
        let handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(45));
            signal.store(true, Ordering::SeqCst)
        });
        run_loop(&cfg, &cancel, &mut io).unwrap();
        handle.join().unwrap();
        assert!(io.scans.len() >= 2);
        assert!(io.actions.is_empty());
    }
    #[test]
    fn capture_error_stops_without_tapping() {
        let mut io = fake(&["box1"]);
        io.fail = true;
        assert_eq!(
            run_loop(&defaults(), &io.cancel.clone(), &mut io).unwrap_err(),
            "capture failed"
        );
        assert!(io.actions.is_empty());
    }
}

#[cfg(test)]
mod live_test {
    use super::*;
    use crate::config::Rule;
    struct Recording<'a> {
        inner: RealIo<'a>,
        scans: usize,
        actions: Vec<String>,
    }
    impl BotIo for Recording<'_> {
        fn scan(
            &mut self,
            rule: &Rule,
            cfg: &Config,
            cancel: &Arc<AtomicBool>,
        ) -> Result<bool, String> {
            self.scans += 1;
            let hit = self.inner.scan(rule, cfg, cancel)?;
            if hit {
                println!("MATCH {}: {}", rule.id, self.inner.text)
            }
            Ok(hit)
        }
        fn found(&mut self, rule: &Rule) {
            self.inner.found(rule)
        }
        fn act(&mut self, rule: &Rule, cancel: &Arc<AtomicBool>) -> Result<(), String> {
            if rule.id == "box1" && self.actions.iter().any(|id| id == "box8") {
                println!("STOP before starting another game after Mystery Box");
                cancel.store(true, Ordering::SeqCst);
                return Ok(());
            }
            println!("TAP {}", rule.id);
            self.inner.act(rule, cancel)?;
            self.actions.push(rule.id.clone());
            Ok(())
        }
    }
    #[test]
    #[ignore = "interacts with a running emulator and taps the game"]
    fn run_live_without_minigame() {
        let mut cfg: Config =
            serde_json::from_str(include_str!("../../../config/cookie-run.default.json")).unwrap();
        cfg.rules.retain(|r| matches!(r.action, Action::Tap { .. }));
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let timer = std::thread::spawn(move || {
            let deadline = Instant::now()
                + Duration::from_secs(
                    std::env::var("COOKIE_RUN_SECONDS")
                        .ok()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(45),
                );
            while Instant::now() < deadline && !stop.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(100));
            }
            stop.store(true, Ordering::SeqCst)
        });
        let mut io = Recording {
            inner: RealIo {
                app: None,
                device: "emulator-5554",
                path: r"C:\LDPlayer\LDPlayer14\adb.exe",
                image: None,
                text: String::new(),
                ocr: WindowsOcr,
            },
            scans: 0,
            actions: vec![],
        };
        let result = run_loop(&cfg, &cancel, &mut io);
        timer.join().unwrap();
        println!(
            "LIVE scans={} actions={:?} result={:?}",
            io.scans, io.actions, result
        );
        if let Err(e) = result {
            if !e.contains("ยกเลิก") {
                panic!("live loop failed: {e}")
            }
        }
        assert!(io.scans > 0, "no scans completed");
    }
}
#[cfg(test)]
mod integration_fixtures {
    use super::*;
    use crate::config::Rule;
    use std::collections::VecDeque;
    struct FixtureIo {
        targets: VecDeque<&'static str>,
        actions: Vec<(String, [u32; 2])>,
        cancel: Arc<AtomicBool>,
        ocr: WindowsOcr,
        directory: std::path::PathBuf,
        scans: usize,
    }
    impl BotIo for FixtureIo {
        fn scan(&mut self, rule: &Rule, cfg: &Config, _: &Arc<AtomicBool>) -> Result<bool, String> {
            self.scans += 1;
            if self.scans > 100 {
                return Err("fixture loop did not complete".into());
            }
            let screen = self
                .directory
                .join(format!("{}.png", self.targets.front().unwrap()));
            let img = image::open(screen).map_err(|e| e.to_string())?;
            let [l, t, r, b] = rule.rect;
            let crop = img.crop_imm(l, t, r - l, b - t).resize_exact(
                (r - l) * cfg.scale,
                (b - t) * cfg.scale,
                image::imageops::FilterType::Lanczos3,
            );
            Ok(cookie_run::matches(
                &rule.label,
                &self.ocr.recognize(&crop)?,
            ))
        }
        fn found(&mut self, _: &Rule) {}
        fn act(&mut self, rule: &Rule, _: &Arc<AtomicBool>) -> Result<(), String> {
            let Action::Tap { point } = rule.action else {
                return Err("minigame excluded".into());
            };
            self.actions.push((rule.id.clone(), point));
            self.targets.pop_front();
            if self.targets.is_empty() {
                self.cancel.store(true, Ordering::SeqCst)
            }
            Ok(())
        }
    }
    #[test]
    #[ignore = "requires Windows OCR"]
    fn full_loop_with_real_ocr_and_simulated_taps() {
        let mut cfg: Config =
            serde_json::from_str(include_str!("../../../config/cookie-run.default.json")).unwrap();
        cfg.interval_ms = 1;
        cfg.post_tap_ms = 0;
        for r in &mut cfg.rules {
            r.wait_ms = 0
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let mut io = FixtureIo {
            targets: ["box1", "box2", "box7", "box8", "box9"]
                .into_iter()
                .collect(),
            actions: vec![],
            cancel: cancel.clone(),
            ocr: WindowsOcr,
            directory: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../tests/ocr-fixtures"),
            scans: 0,
        };
        run_loop(&cfg, &cancel, &mut io).unwrap();
        assert_eq!(
            io.actions,
            vec![
                ("box1".into(), [1195, 807]),
                ("box2".into(), [1123, 764]),
                ("box7".into(), [580, 772]),
                ("box8".into(), [796, 804]),
                ("box9".into(), [793, 794])
            ]
        );
        println!(
            "real OCR, simulated taps: {:?}, scans={}",
            io.actions, io.scans
        );
    }
}
