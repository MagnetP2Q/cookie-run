import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { backend } from "../lib/backend";
import type { BotStatus, LogEntry, DeviceInfo } from "../types";

const logTimeFormatter = new Intl.DateTimeFormat("en-GB", {
  timeZone: "Asia/Bangkok",
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hourCycle: "h23",
});

type IconName =
  | "settings"
  | "refresh"
  | "play"
  | "stop"
  | "minimize"
  | "close"
  | "back"
  | "cookie";
const paths: Record<IconName, string> = {
  settings:
    "M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7ZM19.3 13.4l1.2.9-1.3 2.3-1.5-.5a7.5 7.5 0 0 1-1.6.9l-.3 1.6h-2.6l-.4-1.6a7.5 7.5 0 0 1-1.7-.9l-1.4.5-1.3-2.3 1.2-.9a7.2 7.2 0 0 1 0-1.8l-1.2-.9 1.3-2.3 1.4.5a7.5 7.5 0 0 1 1.7-.9l.4-1.6h2.6l.3 1.6a7.5 7.5 0 0 1 1.6.9l1.5-.5 1.3 2.3-1.2.9a7.2 7.2 0 0 1 0 1.8Z",
  refresh:
    "M20 7v5h-5M4 17v-5h5M5.2 9a7 7 0 0 1 12-2L20 12M4 12l2.8 5a7 7 0 0 0 12-2",
  play: "M8 5v14l11-7-11-7Z",
  stop: "M7 7h10v10H7z",
  minimize: "M5 12h14",
  close: "M6 6l12 12M18 6 6 18",
  back: "M19 12H5m0 0 6-6m-6 6 6 6",
  cookie:
    "M12 3a9 9 0 1 0 9 9 4 4 0 0 1-4-4 4 4 0 0 1-5-5ZM8 10h.01M13 15h.01M7 15h.01",
};
function Icon({ name, size = 18 }: { name: IconName; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d={paths[name]} />
    </svg>
  );
}

function DevicePicker({
  devices,
  value,
  disabled,
  onChange,
}: {
  devices: DeviceInfo[];
  value: string;
  disabled: boolean;
  onChange: (value: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const root = useRef<HTMLDivElement>(null);
  const choose = (item: DeviceInfo) => {
    onChange(item.id);
    setOpen(false);
  };
  const label = (item: DeviceInfo) =>
    item.name ? `${item.id} (${item.name})` : item.id;
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [open]);
  useEffect(() => {
    if (disabled) setOpen(false);
  }, [disabled]);
  useEffect(() => {
    if (open)
      document
        .getElementById(`device-option-${active}`)
        ?.scrollIntoView({ block: "nearest" });
  }, [open, active]);
  const toggle = () => {
    if (!open)
      setActive(
        Math.max(
          0,
          devices.findIndex((item) => item.id === value),
        ),
      );
    setOpen(!open);
  };
  const handleKey = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key === "Escape" && open) {
      event.preventDefault();
      setOpen(false);
      return;
    }
    if (event.key === "Tab") {
      setOpen(false);
      return;
    }
    if (
      event.key === "ArrowDown" ||
      event.key === "ArrowUp" ||
      event.key === "Home" ||
      event.key === "End"
    ) {
      event.preventDefault();
      if (!devices.length) {
        setOpen(true);
        return;
      }
      if (!open) {
        setActive(
          Math.max(
            0,
            devices.findIndex((item) => item.id === value),
          ),
        );
        setOpen(true);
        return;
      }
      setActive((current) =>
        event.key === "Home"
          ? 0
          : event.key === "End"
            ? devices.length - 1
            : event.key === "ArrowDown"
              ? Math.min(current + 1, devices.length - 1)
              : Math.max(current - 1, 0),
      );
      return;
    }
    if ((event.key === "Enter" || event.key === " ") && open) {
      event.preventDefault();
      if (devices[active]) choose(devices[active]);
      else setOpen(false);
    }
  };
  return (
    <div className="select-wrap" ref={root}>
      <button
        id="emulator"
        type="button"
        className={"device-trigger" + (open ? " is-open" : "")}
        role="combobox"
        aria-label="Emulator"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? "device-list" : undefined}
        aria-activedescendant={
          open && devices.length ? `device-option-${active}` : undefined
        }
        disabled={disabled}
        onClick={toggle}
        onKeyDown={handleKey}
      >
        <span
          className={"device-indicator" + (value ? " connected" : "")}
          aria-hidden="true"
        />
        <span className={value ? "device-value" : "device-placeholder"}>
          {devices.find((item) => item.id === value)
            ? label(devices.find((item) => item.id === value)!)
            : value || "เลือก emulator"}
        </span>
        <svg
          className="device-chevron"
          width="15"
          height="15"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="m6 9 6 6 6-6" />
        </svg>
      </button>
      {open && (
        <div
          className="device-menu"
          id="device-list"
          role="listbox"
          aria-label="รายการ emulator"
        >
          {devices.length === 0 ? (
            <div className="device-empty">
              ไม่พบ emulator — กดรีเฟรชเพื่อค้นหา
            </div>
          ) : (
            devices.map((item, index) => (
              <div
                id={`device-option-${index}`}
                key={item.id}
                role="option"
                aria-selected={item.id === value}
                className={
                  "device-option" +
                  (index === active ? " active" : "") +
                  (item.id === value ? " selected" : "")
                }
                onPointerMove={() => setActive(index)}
                onClick={() => choose(item)}
              >
                <span className="option-icon" aria-hidden="true">
                  <svg
                    width="15"
                    height="15"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="1.8"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <rect x="3.5" y="5" width="17" height="12" rx="2" />
                    <path d="M9 20h6m-3-3v3" />
                  </svg>
                </span>
                <span className="option-name">{label(item)}</span>
                {item.id === value && (
                  <span className="option-check" aria-hidden="true">
                    ✓
                  </span>
                )}
              </div>
            ))
          )}
        </div>
      )}
    </div>
  );
}

export default function App() {
  const [status, setStatus] = useState<BotStatus>({
    state: "stopped",
    device: null,
    message: null,
  });
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [device, setDevice] = useState("");
  const [path, setPath] = useState("");
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [logsReady, setLogsReady] = useState(false);
  const [clearingLogs, setClearingLogs] = useState(false);
  const clearInProgress = useRef(false);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const refreshVersion = useRef(0);
  const [view, setView] = useState<"dashboard" | "settings">("dashboard");
  const run = async (fn: () => Promise<unknown>) => {
    setError("");
    setBusy(true);
    try {
      await fn();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const clearLogs = async () => {
    if (clearInProgress.current) return;
    clearInProgress.current = true;
    setClearingLogs(true);
    setError("");
    try {
      await backend.clearLogs();
    } catch (e) {
      setError(String(e));
    } finally {
      clearInProgress.current = false;
      setClearingLogs(false);
    }
  };
  const refresh = async () => {
    const version = ++refreshVersion.current;
    setError("");
    setRefreshing(true);
    try {
      const d = await backend.devices();
      if (version === refreshVersion.current) {
        setDevices(d);
        setDevice((old) =>
          d.some((item) => item.id === old) ? old : d[0]?.id || "",
        );
      }
    } catch (e) {
      if (version === refreshVersion.current) setError(String(e));
    } finally {
      setRefreshing(false);
    }
  };
  const start = async () => {
    refreshVersion.current++;
    await backend.start(device);
  };
  useEffect(() => {
    let active = true;
    const unsubs: Array<() => void> = [];
    const subscribe = async () => {
      try {
        const statusUnsub = await listen<BotStatus>("bot-status", (e) => {
          if (active) setStatus(e.payload);
        });
        if (!active) {
          statusUnsub();
          return;
        }
        unsubs.push(statusUnsub);
        let loading = true;
        let pending: LogEntry[] = [];
        const logUnsub = await listen<LogEntry>("cycle-log", (e) => {
          if (!active) return;
          if (loading) pending.push(e.payload);
          else setLogs((items) => items.some((item) => item.id === e.payload.id) ? items : [...items, e.payload].slice(-100));
        });
        if (!active) { logUnsub(); return; }
        unsubs.push(logUnsub);
        const clearUnsub = await listen("logs-cleared", () => {
          if (active) { pending = []; setLogs([]); }
        });
        if (!active) { clearUnsub(); return; }
        unsubs.push(clearUnsub);
        const errorUnsub = await listen<string>("log-error", (e) => {
          if (active) setError(e.payload);
        });
        if (!active) { errorUnsub(); return; }
        unsubs.push(errorUnsub);
        let history: LogEntry[] = [];
        try { history = await backend.logs(); }
        catch (e) { if (active) setError(`อ่าน log ไม่สำเร็จ: ${String(e)}`); }
        if (active) {
          const seen = new Set(history.map((entry) => entry.id));
          setLogs([...history, ...pending.filter((entry) => !seen.has(entry.id))].slice(-100));
          setLogsReady(true);
        }
        loading = false;
        const [s, c] = await Promise.all([
          backend.status(),
          backend.settings(),
        ]);
        if (active) {
          setStatus(s);
          setPath(c.adb_path);
        }
      } catch (e) {
        if (active) { setError(String(e)); setLogsReady(true); }
      }
    };
    void subscribe();
    return () => {
      active = false;
      unsubs.forEach((f) => f());
    };
  }, []);
  const stopped = status.state === "stopped" || status.state === "error";
  const statusLabel: Record<BotStatus["state"], string> = {
    stopped: "พร้อมเริ่ม",
    starting: "กำลังเริ่ม",
    running: "กำลังทำงาน",
    stopping: "กำลังหยุด",
    error: "มีข้อผิดพลาด",
  };
  const windowAction = (action: "minimize" | "close") => {
    void run(() =>
      action === "close"
        ? getCurrentWindow().close()
        : getCurrentWindow().minimize(),
    );
  };
  return (
    <div className="app-shell">
      <header className="titlebar" data-tauri-drag-region>
        <div className="brand" data-tauri-drag-region>
          <img
            className="brand-app-icon"
            src="/app-icon-32.png"
            width="22"
            height="22"
            alt=""
            data-tauri-drag-region
          />
          <span data-tauri-drag-region>
            COOKIE RUN <strong>BOT</strong>
          </span>
        </div>
        <div className="title-drag" data-tauri-drag-region />
        <div className="window-actions">
          <button
            aria-label="ย่อหน้าต่าง"
            title="ย่อหน้าต่าง"
            onClick={() => windowAction("minimize")}
          >
            <Icon name="minimize" size={16} />
          </button>
          <button
            className="window-close"
            aria-label="ปิดหน้าต่าง"
            title="ปิดหน้าต่าง"
            onClick={() => windowAction("close")}
          >
            <Icon name="close" size={16} />
          </button>
        </div>
      </header>
      <main className="background">
        <div className="content-panel">
          {view === "dashboard" ? (
            <>
              <div className="panel-heading">
                <div>
                  <h1>
                    Cookie Run Bot <span>✦</span>
                  </h1>
                  <p>เลือก emulator และเริ่มต้นการทำงาน</p>
                </div>
                <button
                  className="icon-button settings-button"
                  aria-label="ตั้งค่า ADB"
                  title="ตั้งค่า ADB"
                  onClick={() => setView("settings")}
                >
                  <Icon name="settings" size={20} />
                </button>
              </div>
              <div className="device-row">
                <label htmlFor="emulator">Emulator</label>
                <DevicePicker
                  devices={devices}
                  value={device}
                  disabled={!stopped || busy}
                  onChange={setDevice}
                />
                <button
                  className="icon-button refresh-button"
                  aria-label="ค้นหา emulator"
                  title="ค้นหา emulator"
                  onClick={() => void refresh()}
                  disabled={!stopped || busy || refreshing}
                >
                  <Icon name="refresh" size={18} />
                </button>
              </div>
              <div className="action-row">
                <button
                  className="start-button"
                  disabled={!stopped || !device || busy}
                  onClick={() => run(start)}
                >
                  <span className="action-content">
                    <svg
                      className="action-glyph"
                      width="17"
                      height="17"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="1.7"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      aria-hidden="true"
                    >
                      <path d="M8 5.5a1 1 0 0 1 1.5-.87l9.5 6.5a1.05 1.05 0 0 1 0 1.74l-9.5 6.5A1 1 0 0 1 8 18.5z" />
                    </svg>
                    <span>เริ่มบอท</span>
                  </span>
                </button>
                <button
                  className="stop-button"
                  disabled={stopped || busy || status.state === "stopping"}
                  onClick={() => run(() => backend.stop())}
                >
                  <span className="action-content">
                    <svg
                      className="action-glyph"
                      width="17"
                      height="17"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="1.7"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      aria-hidden="true"
                    >
                      <rect x="6" y="6" width="12" height="12" rx="2" />
                    </svg>
                    <span>หยุด</span>
                  </span>
                </button>
              </div>
              <div className="log-heading">
                <div>
                  <h2>Activity log</h2>
                  <span className={"status " + status.state}>
                    <i />
                    {statusLabel[status.state]}
                  </span>
                </div>
                <button
                  className="clear-button"
                  disabled={!logsReady || clearingLogs}
                  onClick={() => void clearLogs()}
                >
                  ล้าง log
                </button>
              </div>
              <div className="log-list" aria-live="polite">
                {logs.length === 0 ? (
                  <span className="empty">ยังไม่มีรายการ</span>
                ) : (
                  logs
                    .slice(-100)
                    .reverse()
                    .map((l, i) => (
                      <div className="log-entry" key={i}>
                        <time>
                          {logTimeFormatter.format(new Date(l.timestamp * 1000))}
                        </time>
                        <span>{l.message}</span>
                      </div>
                    ))
                )}
              </div>
            </>
          ) : (
            <>
              <div className="panel-heading settings-heading">
                <div>
                  <h1>ตั้งค่า ADB</h1>
                  <p>ระบุตำแหน่งไฟล์ adb.exe</p>
                </div>
                <button
                  className="icon-button back-button"
                  aria-label="กลับหน้าหลัก"
                  title="กลับหน้าหลัก"
                  onClick={() => setView("dashboard")}
                >
                  <Icon name="back" size={19} />
                </button>
              </div>
              <div className="settings-body">
                <label htmlFor="adb-path">ADB path</label>
                <input
                  id="adb-path"
                  value={path}
                  onChange={(e) => setPath(e.target.value)}
                  disabled={!stopped}
                />
                <button
                  className="start-button save-button"
                  disabled={!stopped || busy}
                  onClick={() =>
                    run(async () => {
                      await backend.save(path);
                      setView("dashboard");
                    })
                  }
                >
                  บันทึก
                </button>
                {!stopped && (
                  <p className="settings-note">หยุดบอทก่อนเปลี่ยนการตั้งค่า</p>
                )}
              </div>
            </>
          )}
        </div>
        {(error || status.message) && (
          <div role="alert" className="error-toast">
            {error || status.message}
            {error && (
              <button aria-label="ปิดข้อความ" onClick={() => setError("")}>
                ×
              </button>
            )}
          </div>
        )}
      </main>
    </div>
  );
}
