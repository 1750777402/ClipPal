// 全局类型定义

// 剪贴板记录类型
export interface ClipRecord {
  id: string;
  type: string;
  content: string;
  created: number;
  user_id: number;
  os_type: string;
  fileSize?: number;
  pinned_flag?: number;
  file_info?: FileInfo[];
  image_info?: {
    path: string;
    size: number;
    width?: number;
    height?: number;
  };
  sync_flag?: 0 | 1 | 2; // 0: 未同步, 1: 同步中, 2: 已同步
}

// 文件信息类型
export interface FileInfo {
  path: string;
  size: number;
  type?: string;
}

// 设置类型
interface AppSettings {
  max_records: number;
  show_hotkey: string;
  cloud_sync_enabled: boolean;
  auto_paste_enabled: boolean;
  show_guide: boolean;
}

// 响应式断点类型
type BreakpointSize = 'xs' | 'sm' | 'md' | 'lg' | 'xl' | 'tauriNarrow' | 'tauriWide';

// 设备类型
type DeviceType = 'mobile' | 'tablet' | 'desktop' | 'tauri-narrow' | 'tauri-wide';

// 平台类型
type PlatformType = 'windows' | 'macos' | 'linux' | 'unknown';

// API响应类型
interface ApiResponse<T = any> {
  success: boolean;
  data?: T;
  error?: string;
}

// 组件事件类型
interface ComponentEvents {
  'update:modelValue': [value: boolean];
  'save': [settings: AppSettings];
  'click': [record: ClipRecord];
  'pin': [record: ClipRecord];
  'delete': [record: ClipRecord];
}

// 全局声明，避免未声明变量报错
declare global {
  interface Window {
    // Tauri API 相关
    __TAURI__?: any;
  }
}

export {};