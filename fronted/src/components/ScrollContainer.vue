<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ref, onMounted } from 'vue'

// 菜单数据
const menus = ref(['首页', '分类', '推荐', '设置'])
const activeMenu = ref('0')
const cardRefs = ref<HTMLElement[]>([])
const isLoading = ref(true)

// 卡片数据 - 从后端获取
const cards = ref<ClipRecord[]>([])

interface ClipRecord {
  id: string;
  type: string;
  content: string;
  created: number;
  user_id: number;
  os_type: string;
}

const container = ref<HTMLElement | null>(null)
let eventUnlisten: (() => void) | null = null;

// 从后端获取剪贴板记录
const fetchClipRecords = async () => {
  try {
    isLoading.value = true;
    // 调用Tauri命令获取数据 
    const data: ClipRecord[] = await invoke('get_clip_records');
    cards.value = data;
    console.log('获取数据成功:', cards.value);
  } catch (error) {
    console.error('获取数据失败:', error);
  } finally {
    isLoading.value = false;
  }
};

const initEventListeners = async () => {
  try {
    eventUnlisten = await listen('clip_record_change', () => {
      fetchClipRecords();
    });
  } catch (error) {
    console.error('事件监听失败:', error);
  }
};


// 获取单个记录详情
const fetchClipRecordDetail = async (id: string) => {
  try {
    // 调用Tauri命令获取单个记录 
    const record: ClipRecord = await invoke('get_clip_record_by_id', { id });
    console.log('记录详情:', record);
    // 这里可以打开详情弹窗或执行其他操作
  } catch (error) {
    console.error('获取详情失败:', error);
  }
};

// 滚轮事件处理（添加节流优化）
let lastScrollTime = 0
const handleWheel = (e: WheelEvent) => {
  e.preventDefault()
  const now = Date.now()
  if (now - lastScrollTime < 50) return
  lastScrollTime = now

  if (container.value) {
    container.value.scrollLeft += e.deltaY * 0.5
    updateActiveMenu()
  }
}

// 菜单选择联动
const handleMenuSelect = (index: string) => {
  activeMenu.value = index
  const targetCard = cardRefs.value[Number(index)]
  if (targetCard && container.value) {
    const containerRect = container.value.getBoundingClientRect()
    const cardRect = targetCard.getBoundingClientRect()
    container.value.scrollLeft += (cardRect.left - containerRect.left - 160)
  }
}

// 滚动时更新激活菜单
const updateActiveMenu = () => {
  if (!container.value) return

  const scrollLeft = container.value.scrollLeft
  const cardWidth = 300 + 20 // 卡片宽度+间隙
  const activeIndex = Math.round(scrollLeft / cardWidth)
  activeMenu.value = String(Math.min(activeIndex, cards.value.length - 1))
}

// 格式化时间戳
const formatDate = (timestamp: number) => {
  return new Date(timestamp).toLocaleString();
};

// 初始化
onMounted(() => {
  fetchClipRecords();
  initEventListeners();
  window.addEventListener('resize', () => {
    if (container.value) container.value.scrollLeft = 0
  })
})
</script>

<template>
  <div class="main-container">
    <!-- 顶部固定菜单栏 -->
    <el-menu
        class="fixed-menu"
        mode="horizontal"
        @select="handleMenuSelect"
        :default-active="activeMenu"
    >
      <el-menu-item
          v-for="(item, index) in menus"
          :key="index"
          :index="String(index)"
          :class="{'active-menu-item': activeMenu === String(index)}"
      >
        <div class="menu-item-content">
          <el-icon v-if="index === 0" class="menu-icon"><i class="fas fa-home"></i></el-icon>
          <el-icon v-else-if="index === 1" class="menu-icon"><i class="fas fa-folder"></i></el-icon>
          <el-icon v-else-if="index === 2" class="menu-icon"><i class="fas fa-star"></i></el-icon>
          <el-icon v-else-if="index === 3" class="menu-icon"><i class="fas fa-cog"></i></el-icon>
          <span>{{ item }}</span>
        </div>
      </el-menu-item>
    </el-menu>

    <!-- 加载状态 -->
    <div v-if="isLoading" class="loading-container">
      <div class="spinner-container">
        <div class="spinner"></div>
      </div>
      <p class="loading-text">正在加载剪贴板数据...</p>
    </div>

    <!-- 滚动容器 -->
    <div
        v-else
        class="scroll-container"
        @wheel="handleWheel"
        ref="container"
    >
      <div v-if="cards.length === 0" class="empty-state">
        <el-icon class="empty-icon"><i class="fas fa-clipboard"></i></el-icon>
        <h3>剪贴板历史为空</h3>
        <p>复制一些文本或图像后，它们将出现在这里</p>
        <el-button type="primary" @click="fetchClipRecords">
          <el-icon><i class="fas fa-sync-alt"></i></el-icon>
          <span>刷新数据</span>
        </el-button>
      </div>

      <div v-else class="card-wrapper">
        <div
            v-for="(card, index) in cards"
            :key="card.id"
            class="card"
            @click="fetchClipRecordDetail(card.id)"
            ref="cardRefs"
        >
          <div class="card-header">
            <el-tag 
              :type="card.type === 'text' ? 'success' : 'warning'" 
              size="small"
              class="type-tag"
            >
              {{ card.type === 'text' ? '文本' : '图片' }}
            </el-tag>
            <span class="os-tag">{{ card.os_type }}</span>
          </div>
          
          <h3 class="card-title">{{ card.content.length > 30 ? card.content.slice(0, 30) + '...' : card.content }}</h3>
          
          <div class="card-content">
            <p v-if="card.type === 'text'">{{ card.content }}</p>
            <div v-else class="image-placeholder">
              <el-icon :size="40"><i class="fas fa-image"></i></el-icon>
              <span>图片内容</span>
            </div>
          </div>
          
          <div class="card-footer">
            <span class="timestamp">{{ formatDate(card.created) }}</span>
            <el-tag size="small" class="id-tag">ID:{{ card.id }}</el-tag>
          </div>
        </div>
      </div>
    </div>

    <!-- 底部状态栏 -->
    <div class="app-footer">
      <div class="footer-info">
        <span>
          <el-icon><i class="fas fa-clipboard-list"></i></el-icon>
          总记录: {{ cards.length }}
        </span>
        <span>
          <el-icon><i class="fas fa-thumbtack"></i></el-icon>
          已固定: 0
        </span>
      </div>
      <div class="footer-actions">
        <el-button size="small" class="footer-btn">
          <el-icon><i class="fas fa-trash-alt"></i></el-icon>
          <span>清空</span>
        </el-button>
        <el-button type="primary" size="small" class="footer-btn">
          <el-icon><i class="fas fa-download"></i></el-icon>
          <span>导出</span>
        </el-button>
      </div>
    </div>
  </div>
</template>

<style scoped > 
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
  font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
}

:root {
  --primary: #4361ee;
  --primary-light: #eef1fd;
  --secondary: #6c757d;
  --success: #4caf50;
  --warning: #ff9800;
  --danger: #f44336;
  --light: #f8f9fa;
  --dark: #212529;
  --white: #ffffff;
  --gray-100: #f8f9fa;
  --gray-200: #e9ecef;
  --gray-300: #dee2e6;
  --gray-400: #ced4da;
  --gray-500: #adb5bd;
  --gray-600: #6c757d;
  --gray-700: #495057;
  --gray-800: #343a40;
  --gray-900: #212529;
  --border-radius: 12px;
  --shadow-sm: 0 1px 3px rgba(0, 0, 0, 0.08);
  --shadow-md: 0 4px 12px rgba(0, 0, 0, 0.08);
  --shadow-lg: 0 8px 24px rgba(0, 0, 0, 0.12);
  --transition: all 0.3s ease;
}

.main-container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background-color: var(--gray-100);
  overflow: hidden;
  --menu-height: 70px;
  --submenu-height: 60px;
  --footer-height: 60px;
}

.fixed-menu {
  position: fixed;
  top: 0;
  width: 100%;
  z-index: 1000;
  box-shadow: var(--shadow-sm);
  height: var(--menu-height);
  background-color: var(--white);
  display: flex;
  align-items: center;
  padding: 0 24px;
}

:deep(.el-menu--horizontal) {
  border-bottom: none;
}

:deep(.el-menu--horizontal > .el-menu-item) {
  height: 100%;
  padding: 0 20px;
  border-bottom: none;
  color: var(--gray-600);
  font-weight: 500;
  transition: var(--transition);
}

:deep(.el-menu--horizontal > .el-menu-item:hover) {
  color: var(--primary);
  background-color: transparent;
}

:deep(.el-menu--horizontal > .el-menu-item.is-active) {
  color: var(--primary);
  background-color: transparent;
  border-bottom: none;
}

.active-menu-item {
  position: relative;
  color: var(--primary) !important;
}

.active-menu-item::after {
  content: '';
  position: absolute;
  bottom: 0;
  left: 20px;
  right: 20px;
  height: 3px;
  background: var(--primary);
  border-radius: 3px 3px 0 0;
  transition: var(--transition);
}

.menu-item-content {
  display: flex;
  align-items: center;
  gap: 8px;
}

.menu-icon {
  font-size: 16px;
}

.scroll-container {
  flex: 1;
  width: 100%;
  height: calc(100vh - var(--menu-height) - var(--footer-height));
  margin-top: var(--menu-height);
  overflow-x: auto;
  scroll-behavior: smooth;
  -ms-overflow-style: none;
  scrollbar-width: none;
  padding: 20px 24px;

  &::-webkit-scrollbar {
    display: none;
  }
}

.card-wrapper {
  display: flex;
  gap: 24px;
  min-height: 100%;
  padding: 0 24px;
}

.card {
  flex: 0 0 300px;
  height: 460px;
  /* background: #fff; */
  border-radius: 12px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
  padding: 20px;
  transition:all 0.3s ease;
  display: flex;
  flex-direction: column;
  cursor: pointer;

  &:hover {
    transform: translateY(-5px);
    box-shadow: var(--shadow-lg);
  }
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
  
  .os-tag {
    font-size: 12px;
    font-weight: 500;
    padding: 4px 10px;
    border-radius: 20px;
    background: rgba(67, 97, 238, 0.1);
    color: var(--primary);
  }
}

.type-tag {
  font-weight: 600;
  text-transform: uppercase;
}

.card-title {
  margin: 0 0 15px 0;
  font-size: 18px;
  font-weight: 600;
  color: var(--dark);
  line-height: 1.4;
}

.card-content {
  flex: 1;
  overflow: hidden;
  
  p {
    font-size: 14px;
    color: var(--gray-700);
    line-height: 1.6;
    margin: 0;
    max-height: 280px;
    overflow: auto;
  }
  
  .image-placeholder {
    height: 260px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    background: var(--gray-100);
    border-radius: 8px;
    color: var(--gray-500);
    gap: 12px;
  }
}

.card-footer {
  margin-top: 15px;
  padding-top: 12px;
  border-top: 1px solid var(--gray-200);
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 12px;
  color: var(--gray-600);
}

.id-tag {
  background-color: var(--gray-100);
  color: var(--gray-600);
}

.loading-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: calc(100vh - var(--menu-height) - var(--footer-height));
  margin-top: var(--menu-height);
  gap: 20px;
}

.spinner-container {
  position: relative;
  width: 60px;
  height: 60px;
}

.spinner {
  width: 100%;
  height: 100%;
  border: 4px solid var(--primary-light);
  border-top: 4px solid var(--primary);
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  0% { transform: rotate(0deg); }
  100% { transform: rotate(360deg); }
}

.loading-text {
  font-size: 18px;
  font-weight: 500;
  color: var(--gray-600);
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  text-align: center;
  padding: 40px;
  gap: 20px;
  
  .empty-icon {
    font-size: 64px;
    color: var(--gray-400);
    margin-bottom: 16px;
  }
  
  h3 {
    font-size: 24px;
    font-weight: 600;
    color: var(--gray-700);
  }
  
  p {
    color: var(--gray-600);
    max-width: 400px;
    line-height: 1.6;
  }
}

.app-footer {
  position: fixed;
  bottom: 0;
  left: 0;
  right: 0;
  height: var(--footer-height);
  background: var(--white);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 24px;
  box-shadow: var(--shadow-sm);
  z-index: 100;
}

.footer-info {
  display: flex;
  gap: 24px;
  font-size: 14px;
  color: var(--gray-600);
  
  span {
    display: flex;
    align-items: center;
    gap: 8px;
  }
}

.footer-actions {
  display: flex;
  gap: 12px;
}

.footer-btn {
  display: flex;
  align-items: center;
  gap: 6px;
}

@media (max-width: 768px) {
  .main-container {
    --menu-height: 60px;
    --submenu-height: 50px;
    --footer-height: 50px;
  }
  
  .fixed-menu {
    padding: 0 16px;
  }
  
  :deep(.el-menu--horizontal > .el-menu-item) {
    padding: 0 12px;
    font-size: 14px;
  }
  
  .card {
    flex: 0 0 280px;
    height: 420px;
  }
  
  .card-wrapper {
    padding: 0 16px;
  }
  
  .scroll-container {
    padding: 16px;
  }
  
  .app-footer {
    padding: 0 16px;
  }
  
  .footer-info {
    gap: 16px;
    font-size: 13px;
  }
  
  .empty-state {
    padding: 20px;
    
    .empty-icon {
      font-size: 48px;
    }
    
    h3 {
      font-size: 20px;
    }
  }
}

@media (max-width: 480px) {
  .footer-info {
    display: none;
  }
  
  .footer-actions {
    width: 100%;
    justify-content: space-between;
  }
  
  .footer-btn {
    flex: 1;
    justify-content: center;
  }
  
  :deep(.el-menu--horizontal > .el-menu-item span) {
    display: none;
  }
  
  .menu-item-content {
    justify-content: center;
  }
}
</style>