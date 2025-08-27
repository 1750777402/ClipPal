import { invoke } from '@tauri-apps/api/core';

// 错误类型定义
export enum ErrorSeverity {
  SILENT = 'silent',        // 静默处理，不显示给用户
  INFO = 'info',           // 信息性错误，轻提示
  WARNING = 'warning',     // 警告性错误，需要用户注意
  CRITICAL = 'critical'    // 严重错误，必须显示给用户
}

// 错误分类配置
const ERROR_SEVERITY_MAP: Record<string, ErrorSeverity> = {
  // 数据查询相关 - 静默处理
  'get_clip_records': ErrorSeverity.SILENT,
  'get_image_base64': ErrorSeverity.SILENT,

  // 用户操作相关 - 需要提示
  'copy_clip_record': ErrorSeverity.CRITICAL,
  'copy_clip_record_no_paste': ErrorSeverity.CRITICAL,
  'copy_single_file': ErrorSeverity.CRITICAL,
  'image_save_as': ErrorSeverity.WARNING,
  'del_record': ErrorSeverity.WARNING,
  'set_pinned': ErrorSeverity.INFO,

  // 设置相关 - 严重错误
  'save_settings': ErrorSeverity.CRITICAL,
  'load_settings': ErrorSeverity.SILENT,
  'validate_shortcut': ErrorSeverity.WARNING,

  // 用户认证相关 - 需要提示
  'login': ErrorSeverity.CRITICAL,
  'user_register': ErrorSeverity.CRITICAL,
  'send_email_code': ErrorSeverity.WARNING,
  'logout': ErrorSeverity.INFO,
  'validate_token': ErrorSeverity.SILENT,
  'get_user_info': ErrorSeverity.SILENT,
  'check_login_status': ErrorSeverity.SILENT,
  'update_user_info': ErrorSeverity.WARNING,
  'check_username': ErrorSeverity.SILENT,
};

// API响应类型
export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
  severity: ErrorSeverity;
}

// 错误处理钩子类型
type ErrorHandler = (error: string, severity: ErrorSeverity, command: string) => void;

let globalErrorHandler: ErrorHandler | null = null;

// 设置全局错误处理器
export function setErrorHandler(handler: ErrorHandler) {
  globalErrorHandler = handler;
}

// 统一的API调用封装
export async function apiInvoke<T>(command: string, args?: any): Promise<ApiResponse<T>> {
  try {
    const result = await invoke(command, args);
    return {
      success: true,
      data: result as T,
      severity: ErrorSeverity.SILENT
    };
  } catch (error) {
    const errorMessage = typeof error === 'string' ? error : '操作失败';
    const severity = ERROR_SEVERITY_MAP[command] || ErrorSeverity.INFO;

    // 调用全局错误处理器
    if (globalErrorHandler && severity !== ErrorSeverity.SILENT) {
      globalErrorHandler(errorMessage, severity, command);
    }

    // 在开发环境下，所有错误都打印到控制台
    if (import.meta.env.DEV) {
      console.error(`API Error [${command}]:`, errorMessage);
    }

    return {
      success: false,
      error: errorMessage,
      severity
    };
  }
}

// 特定的API封装函数
export const clipApi = {
  // 查询剪贴记录
  async getClipRecords(params: { page: number; size: number; search?: string }) {
    return apiInvoke<any[]>('get_clip_records', { param: params });
  },

  // 获取图片base64
  async getImageBase64(recordId: string) {
    return apiInvoke<{ id: string; base64_data: string }>('get_image_base64', {
      param: { record_id: recordId }
    });
  },

  // 复制记录
  async copyRecord(recordId: string) {
    return apiInvoke<string>('copy_clip_record', {
      param: { record_id: recordId }
    });
  },

  // 复制记录但不粘贴
  async copyRecordNoPaste(recordId: string) {
    return apiInvoke<string>('copy_clip_record_no_paste', {
      param: { record_id: recordId }
    });
  },

  // 复制单个文件
  async copySingleFile(recordId: string, filePath: string) {
    return apiInvoke<string>('copy_single_file', {
      param: { record_id: recordId, file_path: filePath }
    });
  },

  // 图片另存为
  async imageSaveAs(recordId: string) {
    return apiInvoke<string>('image_save_as', {
      param: { record_id: recordId }
    });
  },

  // 删除记录
  async deleteRecord(recordId: string) {
    return apiInvoke<string>('del_record', {
      param: { record_id: recordId }
    });
  },

  // 设置置顶
  async setPinned(recordId: string, pinnedFlag: number) {
    return apiInvoke<string>('set_pinned', {
      param: { record_id: recordId, pinned_flag: pinnedFlag }
    });
  }
};

// 设置相关API
export const settingsApi = {
  // 加载设置
  async loadSettings() {
    return apiInvoke<any>('load_settings');
  },

  // 保存设置
  async saveSettings(settings: any) {
    return apiInvoke<void>('save_settings', settings);
  },

  // 验证快捷键
  async validateShortcut(shortcut: string) {
    return apiInvoke<boolean>('validate_shortcut', shortcut);
  }
};

// 用户认证相关API
export const userApi = {
  // 用户登录
  async login(params: { account: string; password: string }) {
    return apiInvoke<{
      user_info: any;
      token: string;
      expires_in: string;
    }>('login', { param: params });
  },

  // 用户注册
  async register(params: {
    nickname: string;
    account: string;
    password: string;
    confirmPassword: string;
    email: string;
    captcha: string;
    phone?: string;
  }) {
    // 转换参数名称匹配后端
    const backendParams = {
      nickname: params.nickname,
      account: params.account,
      password: params.password,
      confirm_password: params.confirmPassword, // 转换为下划线命名
      email: params.email,
      captcha: params.captcha,
      phone: params.phone
    };
    return apiInvoke<any>('user_register', { param: backendParams });
  },

  // 用户登出
  async logout() {
    return apiInvoke<string>('logout');
  },

  // 发送邮箱验证码
  async sendEmailCode(params: { email: string }) {
    return apiInvoke<string>('send_email_code', { param: params });
  },

  // 验证Token
  async validateToken() {
    return apiInvoke<boolean>('validate_token');
  },

  // 获取用户信息
  async getUserInfo() {
    return apiInvoke<{
      id: number;
      account: string;
      nickname: string;
      email: string;
      phone: string;
      role: string;
    }>('get_user_info');
  },

  // 检查登录状态（应用启动时调用）
  async checkLoginStatus() {
    return apiInvoke<{
      id: number;
      account: string;
      nickname: string;
      email: string;
      phone: string;
      role: string;
    } | null>('check_login_status');
  },

  // 更新用户信息（暂时保留，后续实现）
  async updateUserInfo(params: {
    nickname?: string;
    email?: string;
    avatar?: string;
  }) {
    return apiInvoke<{ userInfo: any; message?: string }>('update_user_info', { param: params });
  },

  // 检查用户名是否可用
  async checkUsername(params: { username: string }) {
    return apiInvoke<boolean>('check_username', { param: params });
  }
};

// 便捷的成功检查函数
export function isSuccess<T>(response: ApiResponse<T>): response is ApiResponse<T> & { data: T } {
  return response.success && response.data !== undefined;
}

// 获取用户友好的错误消息
export function getFriendlyErrorMessage(error: string, command: string): string {
  // 首先清理错误信息
  const cleanedError = cleanupErrorMessage(error);
  
  // 对于所有带服务器API的操作（登录、注册、验证码等），如果是业务错误信息，直接使用
  const apiCommands = ['login', 'user_register', 'send_email_code', 'logout'];
  const isNetworkError = cleanedError && (cleanedError.includes('连接') || cleanedError.includes('网络') || cleanedError.includes('超时') || cleanedError.includes('DNS') || cleanedError.includes('服务器'));
  
  if (apiCommands.includes(command) && cleanedError && !isNetworkError) {
    // 对于服务器API调用的业务错误，直接显示服务器返回的错误信息
    return cleanedError;
  }

  // 对于本地业务逻辑错误或网络错误，使用默认的友好提示
  const friendlyMessages: Record<string, string> = {
    // 系统设置相关
    'load_settings': '载入设置失败',
    'save_settings': '设置保存失败，请检查配置',
    'validate_shortcut': '快捷键校验失败',
    
    // 剪贴板记录查询
    'get_clip_records': '获取剪贴板记录失败',
    'get_image_base64': '获取图片数据失败',
    
    // 剪贴板记录操作
    'copy_clip_record': '复制失败',
    'copy_clip_record_no_paste': '复制失败',
    'copy_single_file': '复制文件失败',
    'set_pinned': '置顶操作失败',
    'del_record': '删除失败，请重试',
    'image_save_as': '图片保存失败',
    
    // 用户认证相关（网络错误时的备选提示）
    'login': '登录失败，请检查网络或账号密码',
    'user_register': '注册失败，请检查网络或输入信息',
    'send_email_code': '发送验证码失败，请检查网络连接',
    'logout': '登出失败',
    'validate_token': '身份验证失败',
    'get_user_info': '获取用户信息失败',
    'check_login_status': '检查登录状态失败',
    'update_user_info': '更新用户信息失败'
  };

  return friendlyMessages[command] || cleanedError || '操作失败';
}

// 清理错误消息，去除重复和技术细节
function cleanupErrorMessage(error: string): string {
  // 如果错误消息包含层层嵌套的相同信息，只保留最有用的部分
  if (error.includes('云服务异常')) {
    // 提取最核心的错误信息
    const match = error.match(/云服务异常[：:]\s*(.+?)(?:\s*\(.+?\))?$/);
    if (match && match[1]) {
      return `云服务异常: ${match[1]}`;
    }
  }

  if (error.includes('连接失败') || error.includes('无法连接到服务器')) {
    return '无法连接到云服务器，请检查网络连接或稍后重试';
  }

  if (error.includes('连接被拒绝') || error.includes('服务器') && error.includes('未启动')) {
    return '云服务暂时不可用，请稍后重试';
  }

  if (error.includes('请求超时') || error.includes('响应缓慢')) {
    return '网络请求超时，请检查网络连接';
  }

  if (error.includes('DNS解析失败') || error.includes('无法解析域名')) {
    return '网络连接异常，请检查网络设置';
  }

  if (error.includes('服务器异常') && error.includes('返回空响应')) {
    return '云服务暂时不可用，请稍后重试';
  }

  if (error.includes('用户认证已过期')) {
    return '登录已过期，请重新登录';
  }

  if (error.includes('用户未登录')) {
    return '请先登录后再进行此操作';
  }

  // 去除重复的前缀
  let cleaned = error;
  const prefixes = ['通用错误: ', '云服务错误: ', 'HTTP请求错误: ', '网络错误: '];
  for (const prefix of prefixes) {
    if (cleaned.startsWith(prefix)) {
      cleaned = cleaned.substring(prefix.length);
      break;
    }
  }

  return cleaned;
}