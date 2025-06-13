<template>
  <div class="clipboard-panel">
    <header class="panel-header">
      <span class="panel-title">Clip Pal</span>
      <input v-model="search" class="search-input" placeholder="搜索剪贴记录..." />
    </header>

    <div v-if="isLoading" class="loading">加载中...</div>

    <div
      class="clip-list"
      v-else
      @scroll.passive="handleScroll"
      ref="scrollContainer"
    >
      <div
        v-for="item in cards"
        :key="item.id"
        class="clip-card"
      >
        <div class="clip-content" @click="handleCardClick(item)">
          <template v-if="item.type === 'Text'">
            <p
              class="text-preview"
              :class="{ 'mask-visible': shouldShowMask(item.content) }"
              :title="item.content"
            >
              {{ item.content }}
            </p>
          </template>

          <template v-else-if="item.type === 'Image'">
            <div ref="container">
              <InnerImageZoom
                v-if="visible"
                :src="item.content"
                :zoomSrc="item.content"
                :zoomScale="0.7"
                moveType="pan"
                zoomType="hover"
                :fadeDuration="300"
                class="image-preview"
              />
              <div v-else class="image-placeholder">加载中...</div>
            </div>
          </template>

          <template v-else-if="item.type === 'File'">
            <div class="file-preview">
              <div class="file-name" :title="item.content">{{ item.content }}</div>
            </div>
          </template>
        </div>
      </div>

      <div v-if="isFetchingMore" class="bottom-loading">加载更多...</div>
      <div v-if="!hasMore && cards.length > 0" class="bottom-loading">没有更多了</div>
    </div>
  </div>
</template>


<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ref, onMounted, onBeforeUnmount } from 'vue';
import InnerImageZoom from 'vue-inner-image-zoom';

const search = ref('');
const isLoading = ref(false);
const isFetchingMore = ref(false);
const cards = ref<ClipRecord[]>([]);
const page = ref(1);
const pageSize = 20;
const hasMore = ref(true);

const scrollContainer = ref<HTMLElement | null>(null);

interface ClipRecord {
  id: string;
  type: string;
  content: string;
  created: number;
  user_id: number;
  os_type: string;
}

const handleCardClick = async (item: ClipRecord) => {
  await invoke('copy_clip_record', { param: { record_id: item.id } });
};

const shouldShowMask = (text: string) => {
  return text.split('\n').length > 3 || text.length > 100;
};

const initEventListeners = async () => {
  try {
    await listen('clip_record_change', () => {
      resetAndFetch();
    });
  } catch (error) {
    console.error('事件监听失败:', error);
  }
};

const resetAndFetch = () => {
  cards.value = [];
  page.value = 1;
  hasMore.value = true;
  fetchClipRecords();
};

const fetchClipRecords = async () => {
  if (!hasMore.value) return;
  const currentPage = page.value;
  try {
    if (currentPage === 1) isLoading.value = true;
    else isFetchingMore.value = true;

    const data: ClipRecord[] = await invoke('get_clip_records', {
      param: {
        page: currentPage,
        size: pageSize,
      }
    });

    if (data.length < pageSize) hasMore.value = false;
    cards.value.push(...data);
    page.value++;
  } catch (error) {
    console.error('获取数据失败:', error);
  } finally {
    isLoading.value = false;
    isFetchingMore.value = false;
  }
};

const visible = ref(false);
const container = ref<HTMLElement | null>(null);
let observer: IntersectionObserver | null = null;

const handleScroll = () => {
  if (!scrollContainer.value || isFetchingMore.value || !hasMore.value) return;

  const { scrollTop, clientHeight, scrollHeight } = scrollContainer.value;
  if (scrollTop + clientHeight >= scrollHeight - 200) {
    fetchClipRecords();
  }
};

onMounted(() => {
  if ('IntersectionObserver' in window && container.value) {
    observer = new IntersectionObserver(
      (entries) => {
        entries.forEach(entry => {
          if (entry.isIntersecting) {
            visible.value = true;
            observer?.disconnect();
          }
        });
      },
      { threshold: 0.1 }
    );
    observer.observe(container.value);
  } else {
    visible.value = true;
  }

  fetchClipRecords();
  initEventListeners();
});

onBeforeUnmount(() => {
  observer?.disconnect();
});
</script>


<style scoped>

.clipboard-panel {
  width: 100%;
  height: 100%;
  position: fixed;
  top: 0;
  right: 0;
  background: #f5f7fa;
  display: flex;
  flex-direction: column;
  border-left: 1px solid #d1d9e6;
  box-shadow: -2px 0 8px rgba(0, 0, 0, 0.05);
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial;
}

.panel-header {
  padding: 16px;
  display: flex;
  align-items: center;
  gap: 12px;
  background-color: #2c7a7b;
  border-bottom: 1px solid #256d6d;
  color: #e6fffa;
  font-weight: 600;
  font-size: 18px;
  user-select: none;
  flex-shrink: 0;
  min-height: 0;
}

.panel-title {
  display: flex;
  align-items: center;
  gap: 8px;
  white-space: nowrap;
}

.search-input {
  flex: 1;
  padding: 6px 12px;
  border-radius: 8px;
  border: 1px solid #88c0d0;
  font-size: 14px;
  background-color: #e0f2f1;
  color: #004d40;
  transition: border-color 0.2s, box-shadow 0.2s;
}

.search-input::placeholder {
  color: #4a4a4aaa;
}

.search-input:focus {
  outline: none;
  border-color: #319795;
  box-shadow: 0 0 8px rgba(49, 151, 149, 0.4);
  background-color: #ffffff;
  color: #222;
}

.loading {
  padding: 24px;
  text-align: center;
  font-size: 14px;
  color: #666;
}

.clip-list {
  flex: 1;
  overflow-y: auto;
  padding: 12px 0;
  scrollbar-width: thin;
  scrollbar-color: #81e6d9 transparent;
  box-sizing: border-box;
  min-height: 0;
}

.clip-list::-webkit-scrollbar {
  width: 8px;
}
.clip-list::-webkit-scrollbar-thumb {
  background-color: #81e6d9;
  border-radius: 4px;
}
.clip-list::-webkit-scrollbar-track {
  background: transparent;
}

.clip-card {
  background: #ffffff;
  border-radius: 14px;
  box-shadow: 0 2px 10px rgba(38, 50, 56, 0.07);
  padding: 16px;
  margin: 0 20px 16px 20px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  transition: box-shadow 0.3s ease, transform 0.2s ease;
  cursor: pointer;
}

.clip-card:hover {
  box-shadow: 0 8px 18px rgba(38, 50, 56, 0.15);
  transform: translateY(-2px);
}

.clip-content {
  flex: 1;
  overflow: hidden;
  padding-right: 12px;
}

.text-preview {
  font-size: 15px;
  color: #2d3748;
  line-height: 1.5;
  max-height: 4.5em; /* 约3行 */
  overflow: hidden;
  position: relative;
  white-space: normal;
  word-break: break-word;
  padding-right: 6px;
}

.text-preview::after {
  content: "";
  position: absolute;
  bottom: 0;
  left: 0;
  width: 100%;
  height: 2em;
  background: linear-gradient(to bottom, rgba(255, 255, 255, 0), #fff 90%);
  opacity: 0;
  transition: opacity 0.3s ease;
  pointer-events: none;
}

.text-preview.mask-visible::after {
  opacity: 1;
}

.image-placeholder {
  width: 180px;
  height: 120px;
  background-color: #eee;
  border-radius: 12px;
  display: flex;
  justify-content: center;
  align-items: center;
  color: #999;
  font-size: 14px;
}

.image-preview {
  width: 180px;
  max-height: 120px;
  border-radius: 12px;
  object-fit: contain;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  transition: transform 0.3s ease, box-shadow 0.3s ease;
  display: block;
  cursor: pointer;
  margin: 0;
}

.image-preview:hover {
  transform: scale(1.05);
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.15);
}

.file-preview {
  display: flex;
  flex-direction: column;
  justify-content: center;
  padding: 14px 20px;
  font-size: 15px;
  color: #2d3748;
  background-color: #f0f4f8;
  border-radius: 14px;
  box-shadow: 0 1px 6px rgba(50, 60, 70, 0.1);
  word-break: break-word;
  max-width: 300px;
  cursor: default;
  user-select: text;
  line-height: 1.4;
}

.file-name {
  white-space: normal;
  overflow-wrap: break-word;
  word-break: break-word;
}

.file-info {
  margin-top: 6px;
  font-size: 13px;
  color: #718096;
  user-select: none;
}

.bottom-loading {
  padding: 16px;
  text-align: center;
  font-size: 14px;
  color: #666;
}
</style>
