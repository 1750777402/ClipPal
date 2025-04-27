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

    <!-- 滚动容器 -->
    <div
        class="scroll-container"
        @wheel="handleWheel"
        ref="container"
        style="background: #f0f2f5"
    >
      <div class="card-wrapper">
        <div
            v-for="(card, index) in cards"
            :key="index"
            class="card"
            :ref="el => cardRefs[index] = el"
        >
          <h3>{{ card.title }}</h3>
          <p>{{ card.content }}</p>
        </div>
      </div>
    </div>

    <!-- 底部安全区域 -->
    <div class="bottom-spacer"></div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'

// 菜单数据
const menus = ref(['首页', '分类', '推荐', '设置'])
const activeMenu = ref('0')
const cardRefs = ref<HTMLElement[]>([])

// 卡片数据
const cards = ref(
    Array.from({ length: 20 }, (_, i) => ({
      id: i,
      title: `Card ${i + 1}`,
      content: 'Lorem ipsum dolor sit amet...'
    }))
)

const container = ref<HTMLElement | null>(null)

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

// 初始化滚动监听
onMounted(() => {
  window.addEventListener('resize', () => {
    if (container.value) container.value.scrollLeft = 0
  })
})
</script>

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

  &:hover {
    transform: translateY(-5px);
  }
}

.bottom-spacer {
  height: var(--bottom-spacing);
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