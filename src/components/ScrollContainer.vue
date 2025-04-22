<template>
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
        >
          <!-- 卡片内容 -->
          <h3>{{ card.title }}</h3>
          <p>{{ card.content }}</p>
        </div>
      </div>
    </div>
</template>
  
<script setup lang="ts">
  import { ref, onMounted } from 'vue'
  
  // 卡片数据示例
  const cards = ref(
    Array.from({ length: 20 }, (_, i) => ({
      id: i,
      title: `Card ${i + 1}`,
      content: 'Lorem ipsum dolor sit amet...'
    }))
  )
  
  const container = ref<HTMLElement | null>(null)
  
  // 滚轮事件处理 
  const handleWheel = (e: WheelEvent) => {
    e.preventDefault()
    if (!container.value) return
    
    // 将垂直滚动转换为水平滚动
    container.value.scrollLeft += e.deltaY * 0.5
  }
</script>
  
<style scoped>
  .scroll-container {
    width: 100vw;
    height: 500px;
    overflow-x: auto;
    overflow-y: hidden;
    scroll-behavior: smooth;
    -ms-overflow-style: none;
    scrollbar-width: none;
    
    &::-webkit-scrollbar {
      display: none;
    }
  }
  
  .card-wrapper {
    display: flex;
    gap: 20px;
    padding: 0 20px;
    height: 100%;
  }
  
  .card {
    flex: 0 0 300px;
    height: 460px;
    background: #fff;
    border-radius: 12px;
    box-shadow: 0 4px 6px rgba(0,0,0,0.1);
    padding: 20px;
  }
  .bottom-scrollbar {
    position: fixed;
    bottom: 0;
    left: 0;
    width: 100vw;
    height: 8px;
    background: #e0e0e0;
    overflow-x: auto;
    }
</style>