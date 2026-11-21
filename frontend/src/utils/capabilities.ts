import { shallowRef } from 'vue';
import { apiInvoke, isSuccess } from './api';

export interface AppCapabilities {
  edition: 'local' | 'self-hosted' | 'commercial';
  cloud_sync: boolean;
  account: boolean;
  max_records: number;
}

// 能力只由后端决定；没有成功加载时不启用可选入口，也不推断认证方式。
export const appCapabilities = shallowRef<AppCapabilities | null>(null);
let pending: Promise<AppCapabilities | null> | null = null;
export function loadAppCapabilities(refresh = false): Promise<AppCapabilities | null> {
  if (pending) return pending;
  if (appCapabilities.value && !refresh) return Promise.resolve(appCapabilities.value);
  pending = apiInvoke<AppCapabilities>('get_app_capabilities').then(response => {
    if (isSuccess(response)) appCapabilities.value = response.data;
    return appCapabilities.value;
  }).finally(() => { pending = null; });
  return pending;
}
