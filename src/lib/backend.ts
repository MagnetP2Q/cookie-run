import { invoke } from '@tauri-apps/api/core';
import type { BotStatus, Settings, DeviceInfo, LogEntry } from '../types';
export const backend = {
    status: () => invoke<BotStatus>('get_bot_status'), devices: () => invoke<DeviceInfo[]>('list_devices'),
    settings: () => invoke<Settings>('get_settings'), save: (adbPath: string) => invoke<Settings>('save_settings', { adbPath }),
    logs: () => invoke<LogEntry[]>('get_cycle_logs'), clearLogs: () => invoke<void>('clear_logs'),
    start: (device: string) => invoke<void>('start_bot', { device }), stop: () => invoke<void>('stop_bot'),
};