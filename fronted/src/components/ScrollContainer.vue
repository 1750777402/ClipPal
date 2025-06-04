<script setup lang="ts">
import { invoke } from '@tauri-apps/api/tauri';
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

// 从后端获取剪贴板记录
const fetchClipRecords = async () => {
  try {
    isLoading.value = true;
    // 调用Tauri命令获取数据 
    const data: ClipRecord[] = await invoke('get_clip_records');
    cards.value = data;
    
    // 添加模拟数据（实际应用中移除）
    if (data.length === 0) {
      for (let i = 0; i < 20; i++) {
        cards.value.push({
          id: `${i}`,
          type: i % 3 === 0 ? 'text' : 'image',
          content: i % 3 === 0 
            ? `这是第${i}条文本记录，包含重要信息...` 
            : `screenshot-${i}.png`,
          created: Date.now() - i * 3600000,
          user_id: 1001,
          os_type: i % 2 === 0 ? 'macOS' : 'Windows'
        });
      }
    }
  } catch (error) {
    console.error('获取数据失败:', error);
  } finally {
    isLoading.value = false;
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
      >
        {{ item }}
      </el-menu-item>
    </el-menu>

    <!-- 加载状态 -->
    <div v-if="isLoading" class="loading-container">
      <el-icon class="is-loading" size="36">
        <Loading />
      </el-icon>
      <p>正在加载剪贴板数据...</p>
    </div>

    <!-- 滚动容器 -->
    <div
        v-else
        class="scroll-container"
        @wheel="handleWheel"
        ref="container"
        style="background: #f0f2f5"
    >
      <div class="card-wrapper">
        <div
            v-for="(card, index) in cards"
            :key="card.id"
            class="card"
            @click="fetchClipRecordDetail(card.id)"
            ref="cardRefs"
        >
          <div class="card-header">
            <el-tag :type="card.type === 'text' ? 'success' : 'warning'" size="small">
              {{ card.type === 'text' ? '文本' : '图片' }}
            </el-tag>
            <span class="os-tag">{{ card.os_type }}</span>
          </div>
          
          <h3>{{ card.content.length > 30 ? card.content.slice(0, 30) + '...' : card.content }}</h3>
          
          <div class="card-content">
            <p v-if="card.type === 'text'">{{ card.content }}</p>
            <div v-else class="image-placeholder">
              <el-icon :size="40"><Picture /></el-icon>
            </div>
          </div>
          
          <div class="card-footer">
            <span>{{ formatDate(card.created) }}</span>
            <el-tag size="small">ID:{{ card.id }}</el-tag>
          </div>
        </div>
      </div>
    </div>

    <!-- 底部安全区域 -->
    <div class="bottom-spacer"></div>
  </div>
</template>

<style scoped>
.main-container {
  --menu-height: 60px;
  --bottom-spacing: 40px;
}

.fixed-menu {
  position: fixed;
  top: 0;
  width: 100%;
  z-index: 1000;
  box-shadow: 0 2px 12px rgba(0,0,0,0.1);
}

.scroll-container {
  width: 100vw;
  height: calc(100vh - var(--menu-height) - var(--bottom-spacing));
  margin-top: var(--menu-height);
  overflow-x: auto;
  scroll-behavior: smooth;
  -ms-overflow-style: none;
  scrollbar-width: none;
  padding: 20px 0;

  &::-webkit-scrollbar {
    display: none;
  }
}

.card-wrapper {
  display: flex;
  gap: 20px;
  padding: 0 5vw; /* 两侧留白 */
  min-height: 100%;
}

.card {
  flex: 0 0 300px;
  height: 460px;
  background: #fff;
  border-radius: 12px;
  box-shadow: 0 4px 6px rgba(0,0,0,0.1);
  padding: 20px;
  transition: transform 0.3s ease;
  display: flex;
  flex-direction: column;
  cursor: pointer;

  &:hover {
    transform: translateY(-5px);
    box-shadow: 0 6px 12px rgba(0,0,0,0.15);
  }
  
  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
    
    .os-tag {
      font-size: 12px;
      color: #666;
      padding: 2px 8px;
      background: #f5f7fa;
      border-radius: 4px;
    }
  }
  
  h3 {
    margin: 0 0 15px 0;
    font-size: 18px;
    color: #333;
    line-height: 1.4;
  }
  
  .card-content {
    flex: 1;
    overflow: hidden;
    
    p {
      font-size: 14px;
      color: #555;
      line-height: 1.6;
      margin: 0;
      max-height: 280px;
      overflow: auto;
    }
    
    .image-placeholder {
      height: 260px;
      display: flex;
      align-items: center;
      justify-content: center;
      background: #f9f9f9;
      border-radius: 8px;
      color: #999;
    }
  }
  
  .card-footer {
    margin-top: 15px;
    padding-top: 12px;
    border-top: 1px solid #eee;
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 12px;
    color: #888;
  }
}

.bottom-spacer {
  height: var(--bottom-spacing);
}

.loading-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: calc(100vh - var(--menu-height) - var(--bottom-spacing));
  margin-top: var(--menu-height);
  
  p {
    margin-top: 16px;
    color: #666;
  }
}

@media (max-width: 768px) {
  .main-container {
    --menu-height: 50px;
    --bottom-spacing: 30px;
  }

  .card {
    flex: 0 0 280px;
    height: 400px;
  }

  .card-wrapper {
    padding: 0 3vw;
  }
}
</style>