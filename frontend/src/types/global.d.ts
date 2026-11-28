// 全局类型定义

// 剪贴板记录类型
export interface ClipRecord {
  /** 本地自增 ID 的十进制字符串，避免前端整数精度丢失。 */
  id: string;
  type: string;
  content: string;
  created: number;
  fileSize?: number;
  pinned_flag?: number;
  file_info?: FileInfo[];
  sync_skip_reason?: string;
  sync_content_key?: string;
  download_name?: string;
  image_info?: {
    path: string;
    size: number;
    width?: number;
    height?: number;
  };
  content_truncated?: boolean; // 内容是否被截断
  original_content_length?: number; // 原始内容长度
}

export type CardSyncStatus = 'pending_upload' | 'uploading' | 'uploaded' | 'pending_download' | 'downloading';

export interface SyncCardStates {
  enabled: boolean;
  cards: Record<string, CardSyncStatus>;
  incoming: Array<{
    content_key: string;
    name: string;
    created: number;
    status: 'pending_download' | 'downloading';
  }>;
}

// 文件信息类型
export interface FileInfo {
  /** 附件主键；单文件复制传此值，不能使用显示名称定位。 */
  resource_id: string;
  /** 原始显示名称，字段名沿用 path；不是实际磁盘路径。 */
  path: string;
  /** 当前字节数，资源缺失时为 -1。 */
  size: number;
  type?: string;
}

export type AutoPastePermission = 'not_required' | 'required' | 'granted';

// 设置类型
export interface AppSettings {
  /** 设置读取返回的运行状态，不参与持久化。 */
  shortcut_registered?: boolean;
  auto_paste_permission?: AutoPastePermission;
  file_backup_dir?: string | null;
  default_file_backup_dir?: string | null;
  max_records: number;
  auto_start: number;
  shortcut_key: string;
  auto_paste: number;
  tutorial_completed: number;
  bloom_filter_trust_threshold?: number;
  direct_contains_threshold?: number;
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
  status: 'manual' | 'unavailable' | 'up_to_date' | 'available';
  current_version: string;
  latest_version: string | null;
  body: string | null;
  date: string | null;
}

// 更新进度类型
export type UpdateProgress =
  | { phase: 'downloading'; downloaded: number; total: number | null }
  | { phase: 'verifying' }
  | { phase: 'installing' };

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
