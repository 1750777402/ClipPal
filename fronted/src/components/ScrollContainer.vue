<template>
  <div class="clipboard-panel">
    <header class="panel-header">
      <span class="panel-title">
        Clip Pal
      </span>
      <input v-model="search" class="search-input" placeholder="搜索剪贴记录..." />
    </header>

    <div v-if="isLoading" class="loading">加载中...</div>

    <div class="clip-list" v-else>
      <div
        v-for="item in cards"
        :key="item.id"
        class="clip-card"
      >
        <div class="clip-content">
          <template v-if="item.type === 'Text'">
            <p class="text-preview" :title="item.content">{{ item.content }}</p>
          </template>
          <template v-else-if="item.type === 'Image'">
            <img :src="item.content" class="image-preview" />
          </template>
          <template v-else-if="item.type === 'File'">
            <div class="file-preview">
              <span>{{ item.content }}</span>
            </div>
          </template>
        </div>

        <div class="clip-actions">
          <button class="action-button" title="置顶"></button>
          <button class="action-button" title="删除"></button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ref, onMounted } from 'vue'

const search = ref('')
const isLoading = ref(false)
const cards = ref<ClipRecord[]>([])

interface ClipRecord {
  id: string;
  type: string;
  content: string;
  created: number;
  user_id: number;
  os_type: string;
}


let eventUnlisten: (() => void) | null = null;
const initEventListeners = async () => {
  try {
    eventUnlisten = await listen('clip_record_change', () => {
      fetchClipRecords();
    });
  } catch (error) {
    console.error('事件监听失败:', error);
  }
};

// 从后端获取剪贴板记录
const fetchClipRecords = async () => {
  try {
    isLoading.value = true
    // 调用Tauri命令获取数据
    const data: ClipRecord[] = await invoke('get_clip_records')
    cards.value = data
    console.log('获取数据成功:', cards.value)
  } catch (error) {
    console.error('获取数据失败:', error)
  } finally {
    isLoading.value = false
  }
}

// 初始化
onMounted(() => {
  fetchClipRecords();
  initEventListeners();
})
</script>

<style scoped>
.clipboard-panel {
  width: 100%;
  height: 100vh;
  position: fixed;
  top: 0;
  right: 0;
  background: #f9f9f9;
  display: flex;
  flex-direction: column;
  border-left: 1px solid #ddd;
  box-shadow: -2px 0 8px rgba(0, 0, 0, 0.06);
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial;
}
.panel-header {
  padding: 16px;
  display: flex;
  align-items: center;
  gap: 12px;
  background-color: #f1f1f1;
  border-bottom: 1px solid #ddd;
}
.panel-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 16px;
  color: #222;
  white-space: nowrap;
}
.search-input {
  flex: 1;
  padding: 6px 12px;
  border-radius: 8px;
  border: 1px solid #ccc;
  font-size: 14px;
  background-color: #fff;
  transition: border-color 0.2s, box-shadow 0.2s;
}
.search-input:focus {
  outline: none;
  border-color: #3b82f6;
  box-shadow: 0 0 0 2px rgba(59, 130, 246, 0.2);
}
.loading {
  padding: 24px;
  text-align: center;
  font-size: 14px;
  color: #888;
}
.clip-list {
  flex: 1;
  overflow-y: auto;
  padding-top: 12px;
  padding-bottom: 12px;
}
.clip-card {
  background: #fff;
  border-radius: 12px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.03);
  padding: 14px;
  margin: 0 16px 14px 16px; /* 左右各 16px */
  display: flex;
  justify-content: space-between;
  align-items: center;
  transition: box-shadow 0.3s ease;
}
.clip-card:hover {
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.08);
}
.clip-content {
  flex: 1;
  overflow: hidden;
  padding-right: 10px;
}
.text-preview {
  font-size: 14px;
  color: #222;
  line-height: 1.5;
  word-break: break-word;
  white-space: nowrap;         /* 不换行 */
  overflow: hidden;            /* 溢出隐藏 */
  text-overflow: ellipsis;     /* 显示省略号 */
}
.image-preview {
  max-height: 120px;
  max-width: 100%;
  border-radius: 8px;
  object-fit: contain;
}
.file-preview {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 14px;
  color: #333;
}
.clip-actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-left: 10px;
}
.action-button {
  background: none;
  border: none;
  cursor: pointer;
  padding: 6px;
  border-radius: 6px;
  transition: background-color 0.2s;
}
.action-button:hover {
  background-color: #e5e7eb;
}
</style>
