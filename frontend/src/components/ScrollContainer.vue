<template>
  <div class="clipboard-panel">
    <header class="panel-header">
      <span class="panel-title">Clip Pal</span>
      <input v-model="search" class="search-input" placeholder="搜索剪贴记录..." />
      <div class="header-icons">
        <CommercialAccount v-if="CommercialAccount" @change="smartRefresh" />
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

      <!-- 真实数据，透明度渐变 - 正常流式布局 -->
      <div class="real-content" :class="{ 'content-updating': isRefreshing }">
        <div class="virtual-content" :style="{ height: `${virtualTotalHeight}px` }">
          <div
            v-for="virtualItem in virtualItems"
            :key="virtualItem.item.id"
            class="virtual-item"
            :style="{ transform: `translateY(${virtualItem.offsetY}px)` }">
            <ClipCard
              v-memo="[virtualItem.item.id, virtualItem.item.pinned_flag, virtualItem.item.content, virtualItem.item.created, virtualItem.item.file_info]"
              :record="virtualItem.item"
              :is-mobile="responsive.isMobile.value"
              @click="handleCardClick"
              @pin="handlePin"
              @delete="handleDel" />
          </div>
        </div>
      </div>
    </div>

    <!-- 正常数据显示 - 流式布局 -->
    <div class="clip-list" v-else ref="scrollContainer" @scroll.passive="handleScroll">
      <!-- 空状态显示 -->
      <div v-if="cards.length === 0 && !isInitialLoading && !isRefreshing" class="empty-state">
        <div class="empty-text">暂无数据</div>
        <div class="empty-subtitle">剪贴板记录为空</div>
      </div>

      <!-- 卡片列表 -->
      <div class="virtual-content" :style="{ height: `${virtualTotalHeight}px` }">
        <div
          v-for="virtualItem in virtualItems"
          :key="virtualItem.item.id"
          class="virtual-item"
          :style="{ transform: `translateY(${virtualItem.offsetY}px)` }">
          <ClipCard
            v-memo="[virtualItem.item.id, virtualItem.item.pinned_flag, virtualItem.item.content, virtualItem.item.created, virtualItem.item.file_info]"
            :record="virtualItem.item"
            :is-mobile="responsive.isMobile.value"
            @click="handleCardClick"
            @pin="handlePin"
            @delete="handleDel" />
        </div>
      </div>

      <!-- 分页加载状态 -->
      <div v-if="isFetchingMore" class="bottom-loading">
        <div class="loading-spinner small"></div>
        <span>加载更多...</span>
      </div>

      <!-- 没有更多数据提示 -->
      <div v-if="!hasMore && cards.length > 0" class="bottom-loading">没有更多了</div>
    </div>

    <SettingsDialog v-if="showSettings" v-model="showSettings" @save="handleSettingsSave" />
    
    <!-- 回到顶部按钮 -->
    <button 
      v-show="showBackToTop" 
      class="back-to-top-btn" 
      @click="scrollToTop"
      title="回到顶部"
      type="button"
    >
      <span class="iconfont icon-huidaodingbu"></span>
    </button>
  </div>
</template>


<script setup lang="ts">
// 主列表负责搜索、分页和虚拟滚动，卡片负责单条记录的展示与操作。
import { listen } from '@tauri-apps/api/event';
import { ref, onMounted, onBeforeUnmount, watch, nextTick, defineAsyncComponent } from 'vue';
import SettingsDialog from './SettingsDialog.vue';
import ClipCard from './ClipCard.vue';
import { useWindowAdaptive } from '../utils/responsive';
import { clipApi, isSuccess } from '../utils/api';
import { createDebouncedFunction, useMemoryManager } from '../utils/memoryManager';
import { useDynamicVirtualScroll } from '../utils/dynamicVirtualScroll';

// 使用内存安全的防抖函数
// 原有的防抖函数保留以确保兼容性，但推荐使用新的createDebouncedFunction

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
let requestSequence = 0;

// 当前列表只监听统一的本地记录变化事件，刷新频率由 smartRefresh 控制。

const scrollContainer = ref<HTMLElement | null>(null);
const {
  visibleItems: virtualItems,
  totalHeight: virtualTotalHeight,
} = useDynamicVirtualScroll(cards, scrollContainer);

// 回到顶部按钮相关
const showBackToTop = ref(false);

// 使用响应式工具
const responsive = useWindowAdaptive();

// 使用内存管理器
const memoryManager = useMemoryManager();

// 构建时决定是否引入商业前端，单机产物不包含账号/VIP/支付代码。
const CommercialAccount = import.meta.env.VITE_EDITION === 'commercial'
  ? defineAsyncComponent(() => import('../extensions/CommercialAccount.vue')) : null;

/** 列表使用的轻量记录；不包含数据库密文或同步状态。 */
interface ClipRecord {
  // i64 主键通过字符串传输，不能转换成 number 后再调用接口。
  id: string;
  type: string;
  content: string;
  created: number;
  fileSize?: number;
  pinned_flag?: number;
  file_info?: FileInfo[];
  image_info?: ImageInfo;
}

/** 图片按需读取的本地路径和展示元数据。 */
interface ImageInfo {
  path: string;
  size: number;
  width?: number;
  height?: number;
}

/** 每个附件的展示信息，实际磁盘路径由后端根据资源主键解析。 */
interface FileInfo {
  // 同一条记录可有多个同名文件，操作时使用该主键精确选择。
  resource_id: string;
  path: string;
  size: number;
  type?: string;
}

/** 接收卡片点击通知。复制动作已由卡片发起，这里只记录点击。 */
const handleCardClick = async (item: ClipRecord) => {
  // ClipCard组件已经处理了复制逻辑，这里不需要重复调用
  // 可以在这里添加其他需要的逻辑，比如统计、日志等
  console.log('卡片被点击:', item.id);
};

/** 注册记录变化与设置窗口事件，返回所有注销函数供卸载时释放。 */
const initEventListeners = async () => {
  const unlisteners: Array<() => void> = [];
  try {
    unlisteners.push(await listen('clip_record_change', () => { smartRefresh(); }));
    unlisteners.push(await listen('open_settings_windows', () => {
      showSettings.value = true;
    }));
    return unlisteners;
  } catch (error) {
    console.error('事件监听失败:', error);
    unlisteners.forEach((unlisten) => unlisten());
    return [];
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
  const requestId = ++requestSequence;
  const requestSearch = search.value;
  try {
    isRefreshing.value = true;
    const response = await clipApi.getClipRecords({
      page: 1,
      size: pageSize,
      search: search.value
    });
    
    if (requestId === requestSequence && requestSearch === search.value && isSuccess(response)) {
      const data = response.data;
      // 平滑更新数据
      await nextTick();
      cards.value = [...data];
      page.value = 2;
      hasMore.value = data.length >= pageSize;
      lastFetchTime.value = Date.now();
    }
    
  } finally {
    // 延迟移除刷新状态，让动画更平滑
    if (requestId === requestSequence) {
      memoryManager.setTimeout(() => {
        if (requestId === requestSequence) isRefreshing.value = false;
      }, 300);
    }
  }
};

/** 回到列表顶部并重新请求第一页，适用于搜索词变化或空列表刷新。 */
const resetAndFetch = () => {
  page.value = 1;
  hasMore.value = true;
  // 重置虚拟滚动位置
  if (scrollContainer.value) {
    scrollContainer.value.scrollTop = 0;
  }
  fetchClipRecords(true);
};

/** 加载当前页。刷新覆盖列表，加载下一页时追加结果。 */
const fetchClipRecords = async (isRefresh = false) => {
  if (!hasMore.value) return;
  
  const requestId = ++requestSequence;
  const requestSearch = search.value;
  const currentPage = page.value;
  const now = Date.now();
  
  try {
    // 智能loading：只在真正需要时显示
    if (currentPage === 1 && cards.value.length === 0) {
      isInitialLoading.value = true;
    } else if (currentPage > 1) {
      isFetchingMore.value = true;
    }

    const response = await clipApi.getClipRecords({
      page: currentPage,
      size: pageSize,
      search: search.value
    });
    
    // 请求序号和搜索词必须仍匹配，旧响应不能覆盖新一轮搜索。
    if (requestId === requestSequence && requestSearch === search.value && isSuccess(response)) {
      const data = response.data;
      
      if (isRefresh || currentPage === 1) {
        cards.value = [...data];
      } else {
        cards.value.push(...data);
      }
      
      if (data.length < pageSize) hasMore.value = false;
      page.value++;
      lastFetchTime.value = now;
    }
    // 错误处理由API层自动处理，这里不需要额外逻辑
    
  } finally {
    if (requestId === requestSequence) {
      isInitialLoading.value = false;
      isFetchingMore.value = false;
    }
  }
};

/** 更新回顶按钮状态，并在接近底部时提前加载下一页。 */
const handleScroll = () => {
  if (!scrollContainer.value) return;

  const { scrollTop: newScrollTop, clientHeight, scrollHeight } = scrollContainer.value;

  // 控制回到顶部按钮显示
  const threshold = Math.max(clientHeight * 0.8, 300);
  showBackToTop.value = newScrollTop > threshold;

  // 分页加载逻辑 - 滚动到底部时加载下一页
  if (!isFetchingMore.value && hasMore.value && newScrollTop + clientHeight >= scrollHeight - 200) {
    fetchClipRecords();
  }

};

// 回到顶部功能
const scrollToTop = () => {
  if (!scrollContainer.value) return;

  scrollContainer.value.scrollTo({
    top: 0,
    behavior: 'smooth'
  });
};



// 使用内存安全的防抖函数
const searchDebounced = createDebouncedFunction((newValue: string) => {
  // 如果搜索值没有实际变化，不触发请求
  if (newValue === lastSearchValue.value) return;

  lastSearchValue.value = newValue;
  page.value = 1;
  hasMore.value = true;
  // 重置虚拟滚动位置
  if (scrollContainer.value) {
    scrollContainer.value.scrollTop = 0;
  }

  // 搜索时的智能loading
  if (cards.value.length > 0 && newValue.trim()) {
    // 有数据且正在搜索时，使用渐进式更新
    silentRefresh();
  } else {
    // 初始搜索或清空搜索时，正常加载
    fetchClipRecords(true);
  }
}, 500); // 增加防抖时间到500ms，进一步减少请求频率

// 使用内存安全的防抖刷新
const debouncedRefresh = createDebouncedFunction(() => {
  if (cards.value.length > 0) {
    silentRefresh();
  } else {
    resetAndFetch();
  }
}, 500);

watch(search, (newValue) => {
  searchDebounced(newValue);
});

// 响应式设备检测已移到responsive工具中

const showSettings = ref(false);

// 设置更新后只刷新本地列表。
const handleSettingsSave = async () => { smartRefresh(); };

/** 置顶会改变全列表顺序，成功后重新获取第一页。 */
const handlePin = () => {
  smartRefresh();
};

/** 删除后重新查询页面，补齐因删除而前移的记录。 */
const handleDel = () => {
  smartRefresh();
};

let eventUnlisteners: Array<() => void> = [];
let isScrollContainerUnmounted = false;

onMounted(async () => {
  lastSearchValue.value = search.value;
  const registeredListeners = await initEventListeners();
  if (isScrollContainerUnmounted) {
    registeredListeners.forEach((unlisten) => unlisten());
    return;
  }
  eventUnlisteners = registeredListeners;
  void fetchClipRecords();
  

});

onBeforeUnmount(() => {
  isScrollContainerUnmounted = true;
  requestSequence += 1;
  // 响应式监听器在useWindowAdaptive中自动清理

  // 清理防抖函数
  debouncedRefresh.cancel();
  searchDebounced.cancel();

  // 注销本次挂载注册的事件，避免组件销毁后仍触发列表请求。
  eventUnlisteners.forEach((unlisten) => unlisten());
  eventUnlisteners = [];


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
  box-shadow: -2px 0 8px rgba(0, 0, 0, 0.05);
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC', 'Hiragino Sans GB', 'Microsoft YaHei', 'Roboto', 'Helvetica Neue', Arial, sans-serif;
  transition: background-color 0.3s ease;
}

.panel-header {
  padding: var(--spacing-md) var(--spacing-lg);
  display: flex;
  align-items: center;
  gap: var(--spacing-lg);
  background-color: var(--header-bg);
  border-bottom: var(--border-width) solid var(--header-border, #256d6d);
  color: var(--header-text, #e6fffa);
  font-weight: 600;
  font-size: var(--text-lg);
  user-select: none;
  flex-shrink: 0;
  min-height: var(--header-height);
  height: var(--header-height);
  position: sticky;
  top: 0;
  z-index: 10;
  backdrop-filter: blur(8px);
}

.panel-title {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  white-space: nowrap;
  font-size: calc(var(--text-2xl) * 1.7);
  font-weight: 700;
  letter-spacing: 1.2px;
  flex-shrink: 0;
  min-width: 9rem;
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
  color: #ffffff;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'SF Pro Display', 'PingFang SC', 'Microsoft YaHei', sans-serif;
}

.search-input {
  flex: 1;
  max-width: none;
  min-width: 12rem;
  padding: var(--spacing-md) var(--spacing-lg);
  border-radius: 25px; /* 改为更大的圆角，实现圆角输入框效果 */
  border: var(--border-width) solid var(--input-border, #88c0d0);
  font-size: calc(var(--text-base) * 1.6);
  background-color: var(--input-bg, #e0f2f1);
  color: var(--input-text, #004d40);
  transition: all 0.3s ease;
  margin: 0 var(--spacing-md);
  height: calc(var(--input-height) * 1.2);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1); /* 添加轻微阴影增强圆角效果 */
}

.search-input::placeholder {
  color: #333333cc;
  font-size: inherit;
  font-weight: 500;
}

.search-input:focus {
  outline: none;
  border-color: var(--input-focus-border, #319795);
  box-shadow: 0 0 12px rgba(49, 151, 149, 0.2), 0 2px 8px rgba(0, 0, 0, 0.1); /* 保持原有阴影并添加焦点阴影 */
  background-color: var(--input-focus-bg, #ffffff);
  color: var(--input-focus-text, #222);
  transform: translateY(-1px); /* 轻微上移效果增强交互感 */
}

.loading-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-3xl);
  gap: var(--spacing-lg);
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
  padding: var(--spacing-md) 0;
  scrollbar-width: thin;
  scrollbar-color: var(--scrollbar-thumb) transparent;
  box-sizing: border-box;
  min-height: 0;
  -webkit-overflow-scrolling: touch;
  overscroll-behavior: contain;
  position: relative;
}

.virtual-content {
  position: relative;
  width: 100%;
}

.virtual-item {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
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
  background: var(--card-bg);
  border-radius: var(--radius-lg);
  margin: 0 var(--spacing-xl) var(--spacing-lg) var(--spacing-xl);
  padding: var(--spacing-lg);
  border: var(--border-width) solid var(--border-color);
}

.skeleton-header {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  margin-bottom: var(--spacing-md);
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

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-3xl) var(--spacing-xl);
  text-align: center;
  color: var(--text-secondary, #666);
  min-height: 300px;
}

.empty-text {
  font-size: 18px;
  font-weight: 600;
  margin-bottom: var(--spacing-sm);
  color: var(--text-primary, #333);
}

.empty-subtitle {
  font-size: 14px;
  color: var(--text-secondary, #666);
  opacity: 0.8;
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

/* 分批加载样式 */
.batch-loading {
  cursor: pointer;
  transition: all 0.2s ease;
  border-radius: 8px;
  background: var(--batch-bg, #f8fafc);
  border: 1px solid var(--border-color, #e2e8f0);
  margin: 8px 16px;
}

.batch-loading:hover {
  background: var(--batch-hover-bg, #f1f5f9);
  border-color: var(--border-hover-color, #cbd5e1);
}

.batch-info {
  display: flex;
  flex-direction: column;
  gap: 8px;
  align-items: center;
}

.load-more-btn {
  background: var(--primary-color, #2c7a7b);
  color: white;
  border: none;
  border-radius: 6px;
  padding: 6px 16px;
  font-size: 12px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.load-more-btn:hover {
  background: var(--primary-hover-color, #234e52);
  transform: translateY(-1px);
}

.header-icons {
  display: flex;
  gap: var(--spacing-xl);
  align-items: center;
  flex-shrink: 0;
  margin-left: auto;
  padding-left: var(--spacing-lg);
}

.header-icons button.icon-button {
  background: none;
  border: none;
  padding: 4px;
  margin: 0;
  font-size: 20px;
  color: inherit;
  outline: none;
  border-radius: var(--radius-md);
  transition: all 0.2s ease;
}

.icon-button {
  width: 32px;
  height: 32px;
  cursor: pointer;
  transition: all 0.2s ease;
  opacity: 0.9;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-md);
}

.icon-button:hover {
  transform: scale(1.05);
  opacity: 1;
  background-color: rgba(255, 255, 255, 0.12);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
}

.icon-button.logged-in {
  background-color: rgba(255, 255, 255, 0.15);
  box-shadow: 0 0 0 2px rgba(255, 255, 255, 0.3);
  opacity: 1;
}

.icon-button.logged-in:hover {
  background-color: rgba(255, 255, 255, 0.2);
  box-shadow: 0 0 0 2px rgba(255, 255, 255, 0.4);
}

/* 回到顶部按钮 */
.back-to-top-btn {
  position: fixed;
  bottom: 80px;
  right: 24px;
  width: 48px;
  height: 48px;
  background: linear-gradient(135deg, var(--header-bg, #2c7a7b), #319795);
  color: white;
  border: none;
  border-radius: var(--radius-lg);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 18px;
  box-shadow: 0 6px 16px rgba(44, 122, 123, 0.25), 0 2px 8px rgba(0, 0, 0, 0.05);
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  z-index: 1000;
  backdrop-filter: blur(12px);
  border: 1px solid rgba(255, 255, 255, 0.15);
}

.back-to-top-btn:hover {
  background: linear-gradient(135deg, #319795, #2dd4bf);
  box-shadow: 0 8px 20px rgba(44, 122, 123, 0.35), 0 3px 12px rgba(0, 0, 0, 0.1);
  transform: translateY(-2px) scale(1.02);
}

.back-to-top-btn:active {
  transform: translateY(0) scale(0.98);
  box-shadow: 0 3px 8px rgba(44, 122, 123, 0.3), 0 1px 4px rgba(0, 0, 0, 0.05);
}

.back-to-top-btn .iconfont {
  font-size: 18px;
  line-height: 1;
}


/* 响应式布局 */
@media (max-width: 768px) {
  .panel-header {
    padding: 12px 16px;
    gap: var(--spacing-md);
  }

  .panel-title {
    font-size: calc(var(--text-xl) * 1.8);
    font-weight: 700;
    letter-spacing: 1px;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.18);
    min-width: 7rem;
  }

  .search-input {
    flex: 1;
    max-width: none;
    min-width: 10rem;
    padding: var(--spacing-sm) var(--spacing-md);
    font-size: calc(var(--text-sm) * 1.6);
    margin: 0 var(--spacing-sm);
    height: calc(var(--input-height) * 1.4);
    border-radius: 20px; /* 中等屏幕下稍微小一点的圆角 */
  }

  .header-icons {
    gap: var(--spacing-lg);
    margin-left: auto;
    padding-left: var(--spacing-md);
  }

  .icon-button {
    width: 28px;
    height: 28px;
    font-size: 18px;
  }

  .skeleton-card {
    margin: 0 12px 12px 12px;
    padding: 12px;
  }

  /* 空状态响应式调整 */
  .empty-state {
    padding: var(--spacing-2xl) var(--spacing-md);
    min-height: 250px;
  }

  .empty-text {
    font-size: 16px;
  }

  .empty-subtitle {
    font-size: 13px;
  }

  /* 平板尺寸回到顶部按钮调整 */
  .back-to-top-btn {
    width: 44px;
    height: 44px;
    bottom: 72px;
    right: 20px;
    font-size: 16px;
  }
}

/* 中等尺寸窗口优化 */
@media (max-width: 600px) {
  .panel-header {
    padding: 10px 14px;
    gap: var(--spacing-sm);
  }
  
  .panel-title {
    font-size: calc(var(--text-lg) * 1.8);
    font-weight: 700;
    letter-spacing: 0.8px;
    min-width: 75px;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.15);
  }
  
  .search-input {
    flex: 1;
    max-width: none;
    min-width: 8rem;
    padding: var(--spacing-sm) var(--spacing-md);
    font-size: calc(var(--text-sm) * 1.6);
    margin: 0 var(--spacing-xs);
    height: calc(var(--input-height) * 1.2);
    border-radius: 18px; /* 小屏幕下适中的圆角 */
  }
  
  .header-icons {
    gap: var(--spacing-md);
    margin-left: auto;
    padding-left: var(--spacing-sm);
  }
  
  .icon-button {
    width: 26px;
    height: 26px;
    font-size: 16px;
  }
  
  .clip-list {
    padding: 8px 0;
  }

  /* 中等尺寸回到顶部按钮调整 */
  .back-to-top-btn {
    width: 40px;
    height: 40px;
    bottom: 64px;
    right: 18px;
    font-size: 15px;
  }
}

/* 小尺寸窗口优化 */
@media (max-width: 480px) {
  .panel-header {
    padding: 8px 12px;
    gap: var(--spacing-xs);
    min-height: 52px;
  }
  
  .panel-title {
    font-size: calc(var(--text-base) * 1.8);
    font-weight: 700;
    letter-spacing: 0.6px;
    flex-shrink: 0;
    min-width: 75px;
    text-shadow: 0 1px 1px rgba(0, 0, 0, 0.12);
  }
  
  .search-input {
    flex: 1;
    max-width: none;
    min-width: 6rem;
    padding: var(--spacing-xs) var(--spacing-sm);
    font-size: calc(var(--text-xs) * 1.6);
    border-radius: 16px; /* 极小屏幕下保持圆角 */
    margin: 0 calc(var(--spacing-xs) * 0.8);
    height: calc(var(--input-height) * 1.2);
  }
  
  .header-icons {
    gap: var(--spacing-sm);
    margin-left: auto;
    padding-left: var(--spacing-xs);
  }
  
  .icon-button {
    width: 24px;
    height: 24px;
    font-size: 15px;
    opacity: 0.85;
  }
  
  .icon-button:hover {
    opacity: 1;
    transform: scale(1.02);
    background-color: rgba(255, 255, 255, 0.1);
  }
  
  .loading-container {
    padding: 30px 20px;
    gap: 12px;
  }
  
  .loading-spinner {
    width: 32px;
    height: 32px;
    border-width: 2px;
  }
  
  .loading-text {
    font-size: 13px;
  }
  
  .skeleton-card {
    margin: 0 8px 10px 8px;
    padding: 10px;
  }
  
  .bottom-loading {
    padding: 12px;
    font-size: 13px;
  }

  /* 小屏幕空状态调整 */
  .empty-state {
    padding: var(--spacing-xl) var(--spacing-sm);
    min-height: 200px;
  }

  .empty-text {
    font-size: 15px;
  }

  .empty-subtitle {
    font-size: 12px;
  }

  /* 小屏幕回到顶部按钮调整 */
  .back-to-top-btn {
    width: 38px;
    height: 38px;
    bottom: 60px;
    right: 16px;
    font-size: 14px;
  }
}

/* 极小窗口优化 */
@media (max-width: 360px) {
  .panel-header {
    padding: 6px 10px;
    gap: var(--spacing-xs);
    min-height: 48px;
  }
  
  .panel-title {
    font-size: calc(var(--text-sm) * 1.8);
    font-weight: 700;
    letter-spacing: 0.5px;
    min-width: 70px;
    text-shadow: 0 1px 1px rgba(0, 0, 0, 0.1);
  }
  
  .search-input {
    flex: 1;
    max-width: none;
    min-width: 5rem;
    padding: calc(var(--spacing-xs) * 0.8) var(--spacing-xs);
    font-size: calc(var(--text-xs) * 1.6);
    margin: 0 calc(var(--spacing-xs) * 0.6);
    height: calc(var(--input-height) * 0.95);
    border-radius: 14px; /* 极小窗口下的圆角 */
  }
  
  .header-icons {
    gap: calc(var(--spacing-xs) * 0.8);
    margin-left: auto;
    padding-left: calc(var(--spacing-xs) * 0.8);
  }
  
  .icon-button {
    width: 20px;
    height: 20px;
    font-size: 13px;
  }
  
  .skeleton-card {
    margin: 0 6px 8px 6px;
    padding: 8px;
  }
  
  .skeleton-header {
    gap: 8px;
    margin-bottom: 8px;
  }
  
  .skeleton-icon {
    width: 16px;
    height: 16px;
  }
  
  .skeleton-title {
    height: 14px;
  }
  
  .skeleton-time {
    width: 60px;
    height: 10px;
  }
  
  .skeleton-content {
    height: 50px;
  }

  /* 极小屏幕空状态调整 */
  .empty-state {
    padding: var(--spacing-lg) var(--spacing-xs);
    min-height: 180px;
  }

  .empty-text {
    font-size: 14px;
    margin-bottom: calc(var(--spacing-xs) * 0.8);
  }

  .empty-subtitle {
    font-size: 11px;
  }

  /* 极小屏幕回到顶部按钮调整 */
  .back-to-top-btn {
    width: 36px;
    height: 36px;
    bottom: 56px;
    right: 14px;
    font-size: 13px;
  }
}

/* Tauri窗口特殊尺寸优化 - 根据窗口设置优化 */
@media (min-width: 400px) and (max-width: 500px) and (min-height: 600px) {
  /* 针对右侧贴边的窄窗口优化 */
  .clipboard-panel {
    font-size: 14px;
  }
  
  .panel-header {
    padding: 12px 16px;
    gap: var(--spacing-md);
  }
  
  .panel-title {
    font-size: calc(var(--text-lg) * 1.8);
    font-weight: 700;
    letter-spacing: 0.8px;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.15);
  }
  
  .search-input {
    flex: 1;
    max-width: none;
    min-width: 8rem;
    padding: var(--spacing-sm) var(--spacing-md);
    font-size: calc(var(--text-sm) * 1.1);
    margin: 0 var(--spacing-xs);
    height: calc(var(--input-height) * 1.2);
    border-radius: 18px; /* Tauri窗口特殊尺寸下的圆角 */
  }
  
  .header-icons {
    gap: var(--spacing-md);
    margin-left: auto;
    padding-left: var(--spacing-sm);
  }
  
  .icon-button {
    width: 24px;
    height: 24px;
    font-size: 16px;
  }
}

/* 高度限制时的优化 */
@media (max-height: 500px) {
  .panel-header {
    padding: 8px 12px;
    min-height: 40px;
  }
  
  .panel-title {
    font-size: calc(var(--text-lg) * 1.8);
    font-weight: 600;
    letter-spacing: 0.7px;
    text-shadow: 0 1px 1px rgba(0, 0, 0, 0.1);
  }
  
  .search-input {
    padding: 5px 10px;
    font-size: 12px;
    border-radius: 15px; /* 高度限制时的圆角 */
  }
  
  .clip-list {
    padding: 6px 0;
  }
  
  .loading-container {
    padding: 20px;
    gap: 10px;
  }
  
  .skeleton-card {
    padding: 8px 12px;
    margin-bottom: 8px;
  }
}

/* Windows平台特殊优化 */
@media (-ms-high-contrast: none), (-ms-high-contrast: active) {
  /* Windows特定样式 */
  .panel-header {
    backdrop-filter: none;
    background-color: var(--header-bg, #2c7a7b);
  }
  
  .search-input {
    border-width: 1px;
  }
}

/* macOS平台特殊优化 */
@supports (-webkit-backdrop-filter: blur()) {
  @media (max-width: 600px) {
    .panel-header {
      backdrop-filter: blur(20px);
      background-color: rgba(44, 122, 123, 0.9);
    }
  }
}

/* 高DPI显示器优化 */
@media (-webkit-min-device-pixel-ratio: 2), (min-resolution: 192dpi) {
  .panel-header {
    border-bottom-width: 0.5px;
  }
  
  .search-input {
    border-width: 0.5px;
  }
  
  .icon-button {
    border: 0.5px solid transparent;
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
  
  .search-input::placeholder {
    color: #b0b0b0dd;
    font-size: inherit;
    font-weight: 500;
  }
  
  /* 暗色模式下的响应式优化 */
  @media (max-width: 480px) {
    .panel-header {
      background-color: rgba(30, 58, 58, 0.95);
    }
    
      .search-input {
    background-color: #3a3a3a;
    border-color: #4a4a4a;
    border-radius: 16px; /* 暗色模式下的圆角 */
  }
  }

  /* 暗色模式下回到顶部按钮 */
  .back-to-top-btn {
    background: linear-gradient(135deg, #1e3a3a, #2c4a4a);
    box-shadow: 0 6px 16px rgba(30, 58, 58, 0.3), 0 2px 8px rgba(0, 0, 0, 0.1);
  }

  .back-to-top-btn:hover {
    background: linear-gradient(135deg, #2c4a4a, #4a7c7c);
    box-shadow: 0 8px 20px rgba(30, 58, 58, 0.4), 0 3px 12px rgba(0, 0, 0, 0.15);
  }

  /* 暗色模式空状态样式 */
  .empty-state {
    color: var(--text-secondary, #999999);
  }

  .empty-text {
    color: var(--text-primary, #e6e6e6);
  }
}
</style>
