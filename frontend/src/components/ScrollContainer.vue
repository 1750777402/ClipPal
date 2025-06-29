<template>
  <div class="clipboard-panel">
    <header class="panel-header">
      <span class="panel-title">Clip Pal</span>
      <input v-model="search" class="search-input" placeholder="搜索剪贴记录..." />
      <div class="header-icons">
        <button class="icon-button iconfont icon-weitongbu" title="云同步" type="button"></button>
        <button class="icon-button iconfont icon-user" title="用户信息" type="button"></button>
        <button class="icon-button iconfont icon-setting settings-button" title="设置" type="button" @click="showSettings = true"></button>
      </div>
    </header>

    <!-- 只在初始加载和空状态时显示 -->
    <div v-if="isInitialLoading && cards.length === 0" class="loading-container">
      <div class="loading-spinner"></div>
      <span class="loading-text">加载中...</span>
    </div>

    <!-- 骨架屏：在有数据时显示更平滑的加载状态 -->
    <div v-else-if="isRefreshing && cards.length > 0" class="clip-list" @scroll.passive="handleScroll" ref="scrollContainer">
      <div class="skeleton-container">
        <div v-for="n in 3" :key="`skeleton-${n}`" class="skeleton-card">
          <div class="skeleton-header">
            <div class="skeleton-icon"></div>
            <div class="skeleton-title"></div>
            <div class="skeleton-time"></div>
          </div>
          <div class="skeleton-content"></div>
        </div>
      </div>
      
      <!-- 真实数据，透明度渐变 -->
      <div class="real-content" :class="{ 'content-updating': isRefreshing }">
        <ClipCard v-for="item in cards" :key="item.id" :record="item" :is-mobile="isMobile" 
                  @click="handleCardClick" @pin="handlePin" @delete="handleDel" />
      </div>
    </div>

    <!-- 正常数据显示 -->
    <div class="clip-list" v-else @scroll.passive="handleScroll" ref="scrollContainer">
      <ClipCard v-for="item in cards" :key="item.id" :record="item" :is-mobile="isMobile" 
                @click="handleCardClick" @pin="handlePin" @delete="handleDel" />

      <div v-if="isFetchingMore" class="bottom-loading">
        <div class="loading-spinner small"></div>
        <span>加载更多...</span>
      </div>
      <div v-if="!hasMore && cards.length > 0" class="bottom-loading">没有更多了</div>
    </div>

    <SettingsDialog v-model="showSettings" @save="handleSettingsSave" />
  </div>
</template>


<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ref, onMounted, onBeforeUnmount, watch, nextTick } from 'vue';
import SettingsDialog from './SettingsDialog.vue';
import ClipCard from './ClipCard.vue';

// 简单的防抖函数实现
function debounce<T extends (...args: any[]) => any>(func: T, wait: number): (...args: Parameters<T>) => void {
  let timeout: ReturnType<typeof setTimeout> | null = null;
  return function (...args: Parameters<T>) {
    if (timeout) clearTimeout(timeout);
    timeout = setTimeout(() => func(...args), wait);
  };
}

const search = ref('');
const isInitialLoading = ref(false);  // 初始加载状态
const isRefreshing = ref(false);      // 刷新状态
const isFetchingMore = ref(false);
const cards = ref<ClipRecord[]>([]);
const page = ref(1);
const pageSize = 20;
const hasMore = ref(true);

// 缓存机制
const lastFetchTime = ref(0);
const CACHE_DURATION = 2000; // 2秒内避免重复请求
const lastSearchValue = ref('');

const scrollContainer = ref<HTMLElement | null>(null);

interface ClipRecord {
  id: string;
  type: string;
  content: string;
  created: number;
  user_id: number;
  os_type: string;
  fileSize?: number;
}

const handleCardClick = async (item: ClipRecord) => {
  // ClipCard组件已经处理了复制逻辑，这里不需要重复调用
  // 可以在这里添加其他需要的逻辑，比如统计、日志等
  console.log('卡片被点击:', item.id);
};

const initEventListeners = async () => {
  try {
    await listen('clip_record_change', () => {
      smartRefresh();
    });
    await listen('open_settings_winodws', () => {
      showSettings.value = true;
    });
  } catch (error) {
    console.error('事件监听失败:', error);
  }
};

// 智能刷新：避免频繁的loading显示
const smartRefresh = () => {
  const now = Date.now();
  
  // 如果距离上次请求时间太短，使用防抖
  if (now - lastFetchTime.value < CACHE_DURATION) {
    debouncedRefresh();
    return;
  }
  
  // 如果已有数据，使用渐进式更新
  if (cards.value.length > 0) {
    silentRefresh();
  } else {
    resetAndFetch();
  }
};

// 静默刷新：不显示loading，平滑更新数据
const silentRefresh = async () => {
  try {
    isRefreshing.value = true;
    const data: ClipRecord[] = await invoke('get_clip_records', {
      param: {
        page: 1,
        size: pageSize,
        search: search.value
      }
    });
    
    // 平滑更新数据
    await nextTick();
    cards.value = [...data];
    page.value = 2;
    hasMore.value = data.length >= pageSize;
    lastFetchTime.value = Date.now();
    
  } catch (error) {
    console.error('静默刷新失败:', error);
  } finally {
    // 延迟移除刷新状态，让动画更平滑
    setTimeout(() => {
      isRefreshing.value = false;
    }, 300);
  }
};

const resetAndFetch = () => {
  page.value = 1;
  hasMore.value = true;
  fetchClipRecords(true);
};

const fetchClipRecords = async (isRefresh = false) => {
  if (!hasMore.value) return;
  
  const currentPage = page.value;
  const now = Date.now();
  
  try {
    // 智能loading：只在真正需要时显示
    if (currentPage === 1 && cards.value.length === 0) {
      isInitialLoading.value = true;
    } else if (currentPage > 1) {
      isFetchingMore.value = true;
    }

    const data: ClipRecord[] = await invoke('get_clip_records', {
      param: {
        page: currentPage,
        size: pageSize,
        search: search.value
      }
    });
    
    if (isRefresh || currentPage === 1) {
      cards.value = [...data];
    } else {
      cards.value.push(...data);
    }
    
    if (data.length < pageSize) hasMore.value = false;
    page.value++;
    lastFetchTime.value = now;
    
  } catch (error) {
    console.error('获取数据失败:', error);
  } finally {
    isInitialLoading.value = false;
    isFetchingMore.value = false;
  }
};

const handleScroll = () => {
  if (!scrollContainer.value || isFetchingMore.value || !hasMore.value) return;

  const { scrollTop, clientHeight, scrollHeight } = scrollContainer.value;
  if (scrollTop + clientHeight >= scrollHeight - 200) {
    fetchClipRecords();
  }
};

// 优化搜索防抖
const searchDebounced = debounce((newValue: string) => {
  // 如果搜索值没有实际变化，不触发请求
  if (newValue === lastSearchValue.value) return;
  
  lastSearchValue.value = newValue;
  page.value = 1;
  hasMore.value = true;
  
  // 搜索时的智能loading
  if (cards.value.length > 0 && newValue.trim()) {
    // 有数据且正在搜索时，使用渐进式更新
    silentRefresh();
  } else {
    // 初始搜索或清空搜索时，正常加载
    fetchClipRecords(true);
  }
}, 400); // 增加防抖时间，减少请求频率

// 防抖刷新
const debouncedRefresh = debounce(() => {
  if (cards.value.length > 0) {
    silentRefresh();
  } else {
    resetAndFetch();
  }
}, 500);

watch(search, (newValue) => {
  searchDebounced(newValue);
});

// 添加移动设备检测
const isMobile = ref(window.innerWidth <= 768);

// 监听窗口大小变化
const handleResize = debounce(() => {
  isMobile.value = window.innerWidth <= 768;
}, 200);

const showSettings = ref(false);

const handleSettingsSave = async (newSettings: any) => {
  console.log('设置已保存:', newSettings);
};

const handlePin = () => {
  smartRefresh();
};

const handleDel = () => {
  smartRefresh();
};

onMounted(() => {
  window.addEventListener('resize', handleResize);
  lastSearchValue.value = search.value;
  fetchClipRecords();
  initEventListeners();
});

onBeforeUnmount(() => {
  window.removeEventListener('resize', handleResize);
});
</script>


<style scoped>
.clipboard-panel {
  width: 100%;
  height: 100%;
  position: fixed;
  top: 0;
  right: 0;
  background: var(--bg-color, #f5f7fa);
  display: flex;
  flex-direction: column;
  border-left: 1px solid var(--border-color, #d1d9e6);
  box-shadow: -2px 0 8px rgba(0, 0, 0, 0.05);
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC', 'Hiragino Sans GB', 'Microsoft YaHei', 'Roboto', 'Helvetica Neue', Arial, sans-serif;
  transition: background-color 0.3s ease;
}

.panel-header {
  padding: 16px;
  display: flex;
  align-items: center;
  gap: 12px;
  background-color: var(--header-bg, #2c7a7b);
  border-bottom: 1px solid var(--header-border, #256d6d);
  color: var(--header-text, #e6fffa);
  font-weight: 600;
  font-size: clamp(16px, 2vw, 18px);
  user-select: none;
  flex-shrink: 0;
  min-height: 0;
  position: sticky;
  top: 0;
  z-index: 10;
  backdrop-filter: blur(8px);
}

.panel-title {
  display: flex;
  align-items: center;
  gap: 8px;
  white-space: nowrap;
  font-size: 20px;
}

.search-input {
  flex: 1;
  padding: 8px 16px;
  border-radius: 12px;
  border: 1px solid var(--input-border, #88c0d0);
  font-size: 14px;
  background-color: var(--input-bg, #e0f2f1);
  color: var(--input-text, #004d40);
  transition: all 0.3s ease;
  min-width: 0;
}

.search-input::placeholder {
  color: #4a4a4aaa;
}

.search-input:focus {
  outline: none;
  border-color: var(--input-focus-border, #319795);
  box-shadow: 0 0 12px rgba(49, 151, 149, 0.2);
  background-color: var(--input-focus-bg, #ffffff);
  color: var(--input-focus-text, #222);
}

.loading-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px;
  gap: 16px;
}

.loading-spinner {
  width: 40px;
  height: 40px;
  border: 3px solid var(--spinner-border, #e0f2f1);
  border-top-color: var(--spinner-color, #2c7a7b);
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

.loading-spinner.small {
  width: 24px;
  height: 24px;
  border-width: 2px;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.loading-text {
  color: var(--text-secondary, #666);
  font-size: 14px;
}

.clip-list {
  flex: 1;
  overflow-y: auto;
  padding: 12px 0;
  scrollbar-width: thin;
  scrollbar-color: var(--scrollbar-thumb, #81e6d9) transparent;
  box-sizing: border-box;
  min-height: 0;
  -webkit-overflow-scrolling: touch;
  overscroll-behavior: contain;
  position: relative;
}

.clip-list::-webkit-scrollbar {
  width: 6px;
}

.clip-list::-webkit-scrollbar-thumb {
  background-color: var(--scrollbar-thumb, #81e6d9);
  border-radius: 3px;
}

.clip-list::-webkit-scrollbar-track {
  background: transparent;
}

/* 骨架屏样式 */
.skeleton-container {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  z-index: 2;
  background: var(--bg-color, #f5f7fa);
  animation: fadeIn 0.3s ease;
}

.skeleton-card {
  background: var(--card-bg, #ffffff);
  border-radius: 12px;
  margin: 0 20px 16px 20px;
  padding: 16px;
  border: 1px solid var(--border-color, #edf2f7);
}

.skeleton-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}

.skeleton-icon {
  width: 20px;
  height: 20px;
  border-radius: 4px;
  background: linear-gradient(90deg, #f0f0f0 25%, #e0e0e0 50%, #f0f0f0 75%);
  background-size: 200% 100%;
  animation: shimmer 1.5s infinite;
}

.skeleton-title {
  flex: 1;
  height: 16px;
  border-radius: 4px;
  background: linear-gradient(90deg, #f0f0f0 25%, #e0e0e0 50%, #f0f0f0 75%);
  background-size: 200% 100%;
  animation: shimmer 1.5s infinite;
}

.skeleton-time {
  width: 80px;
  height: 12px;
  border-radius: 4px;
  background: linear-gradient(90deg, #f0f0f0 25%, #e0e0e0 50%, #f0f0f0 75%);
  background-size: 200% 100%;
  animation: shimmer 1.5s infinite;
}

.skeleton-content {
  height: 60px;
  border-radius: 8px;
  background: linear-gradient(90deg, #f0f0f0 25%, #e0e0e0 50%, #f0f0f0 75%);
  background-size: 200% 100%;
  animation: shimmer 1.5s infinite;
}

@keyframes shimmer {
  0% {
    background-position: -200% 0;
  }
  100% {
    background-position: 200% 0;
  }
}

.real-content {
  transition: opacity 0.3s ease;
}

.content-updating {
  opacity: 0.6;
}

@keyframes fadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

.bottom-loading {
  padding: 16px;
  text-align: center;
  font-size: 14px;
  color: var(--text-secondary, #666);
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.header-icons {
  display: flex;
  gap: 12px;
  align-items: center;
}

.header-icons button.icon-button {
  background: none;
  border: none;
  padding: 0;
  margin: 0;
  font-size: 18px;
  color: inherit;
  outline: none;
}

.icon-button {
  width: 24px;
  height: 24px;
  cursor: pointer;
  transition: transform 0.2s ease;
  opacity: 0.9;
}

.icon-button:hover {
  transform: scale(1.1);
  opacity: 1;
}

/* 响应式布局 */
@media (max-width: 768px) {
  .panel-header {
    padding: 12px;
    gap: 8px;
  }

  .search-input {
    padding: 6px 12px;
    font-size: 13px;
  }

  .skeleton-card {
    margin: 0 12px 12px 12px;
    padding: 12px;
  }
}

/* 暗色模式支持 */
@media (prefers-color-scheme: dark) {
  .clipboard-panel {
    --bg-color: #1a1a1a;
    --border-color: #2d2d2d;
    --header-bg: #1e3a3a;
    --header-border: #2c4a4a;
    --header-text: #e6fffa;
    --input-bg: #2d2d2d;
    --input-border: #3d3d3d;
    --input-text: #e6e6e6;
    --input-focus-bg: #333333;
    --input-focus-border: #4a9a9a;
    --input-focus-text: #ffffff;
    --card-bg: #2d2d2d;
    --text-primary: #e6e6e6;
    --text-secondary: #999999;
    --scrollbar-thumb: #4a9a9a;
    --spinner-border: #333333;
    --spinner-color: #4a9a9a;
  }
  
  .skeleton-icon,
  .skeleton-title,
  .skeleton-time,
  .skeleton-content {
    background: linear-gradient(90deg, #2d2d2d 25%, #3d3d3d 50%, #2d2d2d 75%);
    background-size: 200% 100%;
  }
}
</style>