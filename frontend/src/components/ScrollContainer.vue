<template>
  <div class="clipboard-panel">
    <header class="panel-header">
      <span class="panel-title">Clip Pal</span>
      <input v-model="search" class="search-input" placeholder="搜索剪贴记录..." />
      <div class="header-icons">
        <button
          :class="['icon-button', 'iconfont', cloudSyncEnabled ? 'icon-yuntongbu' : 'icon-weitongbu']"
          title="云同步"
          type="button"
          @click="handleCloudSyncClick">
        </button>
        <button 
          class="icon-button iconfont icon-user" 
          :class="{ 'logged-in': userStore.isLoggedIn() }"
          :title="userButtonTitle" 
          type="button" 
          @click="handleUserButtonClick"
        ></button>
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
        <ClipCard v-for="item in visibleCards" :key="item.id" :record="item" :is-mobile="responsive.isMobile.value" 
                  :cloud-sync-enabled="cloudSyncEnabled" @click="handleCardClick" @pin="handlePin" @delete="handleDel" />
        
      </div>
    </div>

    <!-- 正常数据显示 -->
    <div class="clip-list" v-else @scroll.passive="handleScroll" ref="scrollContainer">
      <!-- 空状态显示 -->
      <div v-if="cards.length === 0 && !isInitialLoading && !isRefreshing" class="empty-state">
        <div class="empty-text">暂无数据</div>
        <div class="empty-subtitle">剪贴板记录为空</div>
      </div>
      
      <!-- 数据列表 -->
      <ClipCard v-for="item in visibleCards" :key="item.id" :record="item" :is-mobile="responsive.isMobile.value" 
                :cloud-sync-enabled="cloudSyncEnabled" @click="handleCardClick" @pin="handlePin" @delete="handleDel" />


      <!-- 分页加载状态 -->
      <div v-if="isFetchingMore" class="bottom-loading">
        <div class="loading-spinner small"></div>
        <span>加载更多...</span>
      </div>

      <!-- 分批渲染状态 -->
      <div v-if="!isFetchingMore && hasMoreBatches" class="bottom-loading batch-loading" @click="loadNextBatch">
        <div class="batch-info">
          <span>显示 {{ visibleCards.length }} / {{ cards.length }} 条记录</span>
          <button class="load-more-btn">点击加载更多</button>
        </div>
      </div>

      <div v-if="!hasMore && !hasMoreBatches && cards.length > 0" class="bottom-loading">没有更多了</div>
    </div>

    <SettingsDialog v-model="showSettings" @save="handleSettingsSave" />
    
    <!-- 登录对话框 -->
    <LoginDialog v-model:visible="showLoginDialog" @login-success="handleLoginSuccess" />
    
    <!-- 用户菜单 -->
    <UserMenu 
      v-model:visible="showUserMenu" 
      :user-info="userStore.getUserInfo()" 
      @logout="handleLogout" 
      @user-info="handleUserInfo"
      @vip-account="handleVipAccount"
    />
    
    <!-- 用户信息对话框 -->
    <UserInfoDialog 
      v-model:visible="showUserInfoDialog" 
      :user-info="userStore.getUserInfo()" 
      @edit="handleUserInfoEdit"
    />
    
    <!-- VIP账户对话框 -->
    <VipAccountDialog 
      v-model:visible="showVipAccountDialog"
      @login="showLoginDialog = true"
    />
    
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
import { listen } from '@tauri-apps/api/event';
import { ref, onMounted, onBeforeUnmount, watch, nextTick, computed } from 'vue';
import SettingsDialog from './SettingsDialog.vue';
import ClipCard from './ClipCard.vue';
import LoginDialog from './LoginDialog.vue';
import UserMenu from './UserMenu.vue';
import UserInfoDialog from './UserInfoDialog.vue';
import VipAccountDialog from './VipAccountDialog.vue';
import { useWindowAdaptive } from '../utils/responsive';
import { clipApi, settingsApi, isSuccess } from '../utils/api';
import { useUserStore } from '../utils/userStore';
import { createDebouncedFunction, useMemoryManager } from '../utils/memoryManager';

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

// 分批渲染优化 - 大数据集时分批渲染，避免一次性渲染太多DOM
const BATCH_SIZE = 50; // 每批渲染50个卡片
const currentBatch = ref(1);

const visibleCards = computed(() => {
  const totalCards = cards.value;
  // 如果数据量小于100，直接渲染全部
  if (totalCards.length <= 100) {
    return totalCards;
  }

  // 大数据集时分批渲染
  const endIndex = Math.min(currentBatch.value * BATCH_SIZE, totalCards.length);
  return totalCards.slice(0, endIndex);
});

// 是否还有更多数据可以渲染
const hasMoreBatches = computed(() => {
  return currentBatch.value * BATCH_SIZE < cards.value.length;
});

// 缓存机制
const lastFetchTime = ref(0);
const CACHE_DURATION = 2000; // 2秒内避免重复请求
const lastSearchValue = ref('');

// 防止单记录更新后立即全局刷新的防抖机制
const recentlyUpdatedRecords = new Set<string>();
const SINGLE_UPDATE_COOLDOWN = 1000; // 1秒内不重复刷新

const scrollContainer = ref<HTMLElement | null>(null);

// 回到顶部按钮相关
const showBackToTop = ref(false);

// 使用响应式工具
const responsive = useWindowAdaptive();

// 使用内存管理器
const memoryManager = useMemoryManager();

// 用户状态管理
const userStore = useUserStore();

// 登录对话框显示状态
const showLoginDialog = ref(false);

// 用户菜单显示状态
const showUserMenu = ref(false);

// 用户信息对话框显示状态
const showUserInfoDialog = ref(false);

// VIP账户对话框显示状态
const showVipAccountDialog = ref(false);

// 用户按钮提示文本
const userButtonTitle = computed(() => {
  if (userStore.isLoggedIn()) {
    const userInfo = userStore.getUserInfo();
    return `已登录: ${userInfo?.account || '用户'}`;
  }
  return '点击登录';
});

interface ClipRecord {
  id: string;
  type: string;
  content: string;
  created: number;
  user_id: number;
  os_type: string;
  fileSize?: number;
  pinned_flag?: number;
  file_info?: FileInfo[];
  image_info?: ImageInfo;
  sync_flag?: 0 | 1 | 2 | 3; // 0未同步 1同步中 2已同步 3跳过同步
  cloud_source?: 0 | 1; // 0本地数据 1云端同步数据
}

interface ImageInfo {
  path: string;
  size: number;
  width?: number;
  height?: number;
}

interface FileInfo {
  path: string;
  size: number;
  type?: string;
}

const handleCardClick = async (item: ClipRecord) => {
  // ClipCard组件已经处理了复制逻辑，这里不需要重复调用
  // 可以在这里添加其他需要的逻辑，比如统计、日志等
  console.log('卡片被点击:', item.id);
};

const initEventListeners = async () => {
  try {
    await listen('clip_record_change', () => {
      // 如果没有最近的单记录更新，则执行智能刷新
      if (recentlyUpdatedRecords.size === 0) {
        console.log('执行通用刷新：无最近单记录更新');
        smartRefresh();
      } else {
        console.log('跳过通用刷新：有最近的单记录更新', Array.from(recentlyUpdatedRecords));
      }
    });
    await listen('open_settings_windows', () => {
      showSettings.value = true;
    });
    // 单个同步状态更新
    await listen('sync_status_update', (event) => {
      const { clip_id, sync_flag } = event.payload as { clip_id: string, sync_flag: 0 | 1 | 2 | 3 };
      const card = cards.value.find(c => c.id === clip_id);
      if (card) card.sync_flag = sync_flag;
    });
    // 批量同步状态更新
    await listen('sync_status_update_batch', (event) => {
      const { clip_ids, sync_flag } = event.payload as { clip_ids: string[], sync_flag: 0 | 1 | 2 | 3 };
      // 批量更新卡片状态
      clip_ids.forEach(clip_id => {
        const card = cards.value.find(c => c.id === clip_id);
        if (card) card.sync_flag = sync_flag;
      });
    });
    // 云文件下载完成事件 - 单记录更新，提供更好的用户体验
    await listen('clip_record_download_completed', (event) => {
      const { record_id, filename, path, file_info } = event.payload as { 
        record_id: string, 
        action: string, 
        filename: string, 
        path: string,
        file_info?: FileInfo[]
      };
      console.log('收到单个记录下载完成事件:', record_id, { filename, path, file_info });
      
      // 找到对应的记录并更新其状态
      const cardIndex = cards.value.findIndex(c => c.id === record_id);
      if (cardIndex !== -1) {
        const card = cards.value[cardIndex];
        
        // 更新同步状态为已完成
        card.sync_flag = 2;
        
        // 对于文件类型，使用后端提供的file_info数据
        if (card.type === 'File' && file_info) {
          card.content = filename;
          card.file_info = file_info;
          console.log('文件记录已更新:', record_id, { content: filename, file_info });
        } else {
          // 对于非文件类型（如图片），只更新同步状态
          console.log('非文件记录状态已更新:', record_id);
        }
        
        // 强制触发响应式更新
        cards.value = [...cards.value];
        
        // 标记该记录最近被更新，防止不必要的全局刷新
        recentlyUpdatedRecords.add(record_id);
        memoryManager.setTimeout(() => {
          recentlyUpdatedRecords.delete(record_id);
        }, SINGLE_UPDATE_COOLDOWN);
        
      } else {
        // 如果找不到记录，可能是因为记录不在当前页面中（分页问题）
        // 不立即刷新，而是标记为已处理，避免触发不必要的全局刷新
        console.log('未找到对应记录，可能在其他页面，标记为已处理:', record_id);
        
        // 标记该记录最近被更新，防止后续的clip_record_change事件触发全局刷新
        recentlyUpdatedRecords.add(record_id);
        memoryManager.setTimeout(() => {
          recentlyUpdatedRecords.delete(record_id);
        }, SINGLE_UPDATE_COOLDOWN);
      }
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
    const response = await clipApi.getClipRecords({
      page: 1,
      size: pageSize,
      search: search.value
    });
    
    if (isSuccess(response)) {
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
    memoryManager.setTimeout(() => {
      isRefreshing.value = false;
    }, 300);
  }
};

const resetAndFetch = () => {
  page.value = 1;
  hasMore.value = true;
  // 重置分批渲染状态
  currentBatch.value = 1;
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

    const response = await clipApi.getClipRecords({
      page: currentPage,
      size: pageSize,
      search: search.value
    });
    
    // 使用新的API封装，静默处理失败
    if (isSuccess(response)) {
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
    isInitialLoading.value = false;
    isFetchingMore.value = false;
  }
};

const handleScroll = () => {
  if (!scrollContainer.value) return;

  const { scrollTop, clientHeight, scrollHeight } = scrollContainer.value;

  // 控制回到顶部按钮显示
  const threshold = Math.max(clientHeight * 0.8, 300);
  showBackToTop.value = scrollTop > threshold;

  // 原有的分页加载逻辑
  if (!isFetchingMore.value && hasMore.value && scrollTop + clientHeight >= scrollHeight - 200) {
    fetchClipRecords();
  }

  // 新增：分批渲染逻辑 - 滚动到底部时加载下一批
  if (hasMoreBatches.value && scrollTop + clientHeight >= scrollHeight - 100) {
    // 使用setTimeout避免阻塞滚动
    memoryManager.setTimeout(() => {
      currentBatch.value++;
    }, 50);
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

// 手动加载下一批数据
const loadNextBatch = () => {
  if (hasMoreBatches.value) {
    currentBatch.value++;
  }
};

// 使用内存安全的防抖函数
const searchDebounced = createDebouncedFunction((newValue: string) => {
  // 如果搜索值没有实际变化，不触发请求
  if (newValue === lastSearchValue.value) return;

  lastSearchValue.value = newValue;
  page.value = 1;
  hasMore.value = true;
  // 重置分批渲染状态
  currentBatch.value = 1;

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

const cloudSyncEnabled = ref(false);
let cloudSyncDisabledListener: (() => void) | null = null;

// 页面加载时自动加载设置
const loadCloudSyncSetting = async () => {
  const response = await settingsApi.loadSettings();
  if (isSuccess(response)) {
    cloudSyncEnabled.value = response.data.cloud_sync === 1;
  } else {
    cloudSyncEnabled.value = false;
  }
};

// 顶部云同步按钮点击
const handleCloudSyncClick = async () => {
  // 如果要开启云同步，先检查登录状态
  if (!cloudSyncEnabled.value) {
    // 用户尝试开启云同步，检查是否已登录
    if (!userStore.isLoggedIn()) {
      // 用户未登录，显示登录对话框
      showLoginDialog.value = true;
      return;
    }
  }
  
  const loadResponse = await settingsApi.loadSettings();
  if (!isSuccess(loadResponse)) return;
  
  const settings = loadResponse.data;
  const newValue = cloudSyncEnabled.value ? 0 : 1;
  settings.cloud_sync = newValue;
  
  const saveResponse = await settingsApi.saveSettings({ settings });
  if (isSuccess(saveResponse)) {
    cloudSyncEnabled.value = newValue === 1;
    smartRefresh();
  } else {
    // 保存失败，显示具体错误信息
    console.error('云同步设置失败:', saveResponse.error || '未知错误');
    // 这里可以添加用户提示，比如使用消息条或弹窗
    alert(saveResponse.error || '云同步设置失败，请重试');
  }
};

// 设置弹窗保存后同步cloudSyncEnabled
const handleSettingsSave = async (newSettings: any) => {
  cloudSyncEnabled.value = newSettings.cloud_sync === 1;
  smartRefresh();
};

const handlePin = () => {
  smartRefresh();
};

const handleDel = () => {
  smartRefresh();
};

// 用户按钮点击处理
const handleUserButtonClick = () => {
  if (userStore.isLoggedIn()) {
    // 已登录，显示用户菜单
    showUserMenu.value = true;
  } else {
    // 未登录，显示登录对话框
    showLoginDialog.value = true;
  }
};

// 登录成功处理
const handleLoginSuccess = (userData: any) => {
  // 后端已经存储了token，只需要更新前端显示状态
  userStore.setLoginState(userData.user_info);
  // 登录成功后刷新数据
  smartRefresh();
};

// 用户登出处理
const handleLogout = async () => {
  try {
    const success = await userStore.logout();
    if (success) {
      // 登出成功后刷新数据
      smartRefresh();
      // 可选：显示登出成功消息
      console.log('用户已登出');
    }
  } catch (error) {
    console.error('登出失败:', error);
    // 即使登出请求失败，也清除本地状态
    userStore.clearLoginState();
    smartRefresh();
  }
};

// 用户信息处理
const handleUserInfo = () => {
  showUserInfoDialog.value = true;
};

// VIP账户处理
const handleVipAccount = () => {
  showVipAccountDialog.value = true;
};

// 用户信息编辑处理
const handleUserInfoEdit = () => {
  // TODO: 这里可以实现用户信息编辑功能
  console.log('用户信息编辑功能，待实现');
};

onMounted(async () => {
  lastSearchValue.value = search.value;
  fetchClipRecords();
  initEventListeners();
  loadCloudSyncSetting();
  
  // 初始化用户状态
  await userStore.initialize();

  // 监听云同步禁用事件
  cloudSyncDisabledListener = await listen('cloud-sync-disabled', async () => {
    console.log('主页面收到云同步禁用事件，更新云同步状态');
    // 重新加载云同步设置状态
    await loadCloudSyncSetting();
  });
});

onBeforeUnmount(() => {
  // 响应式监听器在useWindowAdaptive中自动清理
  
  // 清理防抖函数
  debouncedRefresh.cancel();
  searchDebounced.cancel();
  
  // 清理Set中的所有项目，防止内存泄漏
  recentlyUpdatedRecords.clear();
  
  // 清理云同步事件监听器
  if (cloudSyncDisabledListener) {
    try {
      cloudSyncDisabledListener();
    } catch (error) {
      console.warn('清理cloudSyncDisabledListener失败:', error);
    }
    cloudSyncDisabledListener = null;
  }
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