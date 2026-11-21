// 全局类型定义

// 剪贴板记录类型
export interface ClipRecord {
  id: string;
  type: string;
  content: string;
  created: number;
  user_id?: number;
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
  sync_flag?: 0 | 1 | 2 | 3; // 0: 未同步, 1: 同步中, 2: 已同步, 3: 不支持同步
  cloud_source?: 0 | 1; // 0: 本地数据, 1: 云端同步数据
  content_truncated?: boolean; // 内容是否被截断
  original_content_length?: number; // 原始内容长度
}

// 文件信息类型
export interface FileInfo {
  path: string;
  size: number;
  type?: string;
}

// 设置类型
export interface AppSettings {
  /** 设置读取返回的运行状态，不参与持久化。 */
  shortcut_registered?: boolean;
  max_records: number;
  auto_start: number;
  shortcut_key: string;
  cloud_sync: number;
  auto_paste: number;
  tutorial_completed: number;
  bloom_filter_trust_threshold?: number;
  direct_contains_threshold?: number;
  cloud_sync_interval: number;
}

export interface UserInfoDto {
  id: number;
  account: string;
  nickname: string;
  email: string;
  phone: string;
  role?: string;
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
  code: string;
  message: string;
  data?: T;
  error?: string;
}

// 更新信息类型
export interface UpdateInfo {
  has_update: boolean;
  current_version: string;
  latest_version: string;
  body?: string;
  size?: number;
  date?: string;
}

// 更新进度类型
export interface UpdateProgress {
  downloaded: number;
  total: number;
  percentage: number;
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

export { };
