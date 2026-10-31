export type BotStatus = { state: 'stopped' | 'starting' | 'running' | 'stopping' | 'error'; device: string | null; message: string | null };
export type Settings = { adb_path: string; config_path: string };
export type LogEntry = { level: string; message: string; timestamp: number };
export type DeviceInfo = { id: string; name: string | null };
