use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};
#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    pub adb_path: String,
    pub config_path: String,
}
#[derive(Clone, Deserialize)]
pub struct Config {
    pub interval_ms: u64,
    pub post_tap_ms: u64,
    pub scale: u32,
    pub rules: Vec<Rule>,
}
#[derive(Clone, Deserialize)]
pub struct Rule {
    pub id: String,
    pub label: String,
    pub rect: [u32; 4],
    pub action: Action,
    #[serde(default)]
    pub states: HashMap<String, bool>,
    #[serde(default)]
    pub wait_ms: u64,
}
#[derive(Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Tap { point: [u32; 2] },
    Minigame { region: [u32; 4] },
}
pub fn init(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join("cookie-run.json");
    if !path.exists() {
        fs::write(
            path,
            include_str!("../../../config/cookie-run.default.json"),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub fn settings(dir: &Path) -> Result<Settings, String> {
    init(dir)?;
    let path = dir.join("settings.json");
    let adb_path = if path.exists() {
        serde_json::from_slice::<Settings>(&fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
            .adb_path
    } else {
        r"C:\LDPlayer\LDPlayer14\adb.exe".to_string()
    };
    Ok(Settings {
        adb_path,
        config_path: dir.join("cookie-run.json").to_string_lossy().into_owned(),
    })
}
pub fn save(dir: &Path, adb_path: String) -> Result<Settings, String> {
    if !PathBuf::from(&adb_path).is_file() {
        return Err("ไม่พบไฟล์ ADB ที่ระบุ".into());
    }
    let mut s = settings(dir)?;
    s.adb_path = adb_path;
    fs::write(
        dir.join("settings.json"),
        serde_json::to_vec_pretty(&s).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(s)
}
pub fn load(path: &str) -> Result<Config, String> {
    let c: Config =
        serde_json::from_slice(&fs::read(path).map_err(|e| format!("อ่าน config ไม่ได้: {e}"))?)
            .map_err(|e| format!("config ไม่ถูกต้อง: {e}"))?;
    if c.interval_ms == 0 || !(1..=4).contains(&c.scale) || c.rules.is_empty() {
        return Err("ค่า interval, scale หรือ rules ไม่ถูกต้อง".into());
    }
    let mut ids = HashSet::new();
    for r in &c.rules {
        if r.id.is_empty() || r.label.is_empty() || !ids.insert(&r.id) || !valid_rect(r.rect) {
            return Err(format!("กฎ {} ไม่ถูกต้อง", r.id));
        }
        match r.action {
            Action::Tap { point } if point[0] >= 1600 || point[1] >= 900 => {
                return Err("พิกัด tap ไม่ถูกต้อง".into())
            }
            Action::Minigame { region } if !valid_rect(region) => {
                return Err("พื้นที่ minigame ไม่ถูกต้อง".into())
            }
            _ => (),
        }
    }
    for r in &c.rules {
        if r.states.keys().any(|k| !ids.contains(k)) {
            return Err(format!("กฎ {} อ้างถึง id ที่ไม่มี", r.id));
        }
    }
    Ok(c)
}
fn valid_rect(r: [u32; 4]) -> bool {
    r[0] < r[2] && r[1] < r[3] && r[2] <= 1600 && r[3] <= 900
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_parse_and_reference_existing_rules() {
        let config: Config =
            serde_json::from_str(include_str!("../../../config/cookie-run.default.json")).unwrap();
        assert_eq!(config.rules.len(), 6);
        assert_eq!(config.rules[1].label, "Play!");
        assert!(config.rules.iter().all(|r| valid_rect(r.rect)));
        for rule in &config.rules {
            for id in rule.states.keys() {
                assert!(config.rules.iter().any(|r| &r.id == id));
            }
        }
    }
    #[test]
    fn rejects_invalid_rectangles() {
        assert!(!valid_rect([0, 0, 1601, 900]));
        assert!(!valid_rect([1, 1, 1, 5]));
    }
}
