<template>
  <div class="smart-content-display">
    <!-- 内容类型指示器 -->
    <div class="content-type-indicator" v-if="showTypeIndicator && detectedContent.type.type !== 'text'">
      <div class="type-badge" :class="`type-${detectedContent.type.type}`">
        <i class="type-icon" :class="getTypeIcon(detectedContent.type)"></i>
        <span class="type-text">{{ getTypeDisplayName(detectedContent.type) }}</span>
        <span class="confidence-text" v-if="detectedContent.type.confidence < 0.8">
          {{ Math.round(detectedContent.type.confidence * 100) }}%
        </span>
      </div>
    </div>

    <!-- 内容显示区域 -->
    <div class="content-display-area" :class="{
      'is-expanded': isExpanded,
      'has-overlay': shouldShowOverlay !== 'none',
      'overlay-partial': shouldShowOverlay === 'partial',
      'overlay-full': shouldShowOverlay === 'full',
      'scroll-visible': showScrollbar
    }" ref="contentArea">
      
      <!-- JSON 内容 -->
      <div v-if="detectedContent.type.type === 'json'" class="json-content">
        <div class="content-header">
          <span class="content-label">JSON 数据</span>
          <button class="copy-btn" @click="copyContent" title="复制格式化内容">
            <i class="iconfont icon-copy"></i>
          </button>
        </div>
        <pre class="formatted-code" v-html="highlightedContent" ref="codeElement"></pre>
      </div>

      <!-- Markdown 内容 -->
      <div v-else-if="detectedContent.type.type === 'markdown'" class="markdown-content">
        <div class="content-header">
          <span class="content-label">Markdown 文档</span>
          <div class="markdown-controls">
            <button :class="{ active: markdownViewMode === 'preview' }" 
                    @click="markdownViewMode = 'preview'" class="view-btn">预览</button>
            <button :class="{ active: markdownViewMode === 'code' }" 
                    @click="markdownViewMode = 'code'" class="view-btn">源码</button>
            <button class="copy-btn" @click="copyContent" title="复制Markdown">
              <i class="iconfont icon-copy"></i>
            </button>
          </div>
        </div>
        <div v-if="markdownViewMode === 'preview'" class="markdown-preview-content" v-html="renderedMarkdown"></div>
        <pre v-else class="formatted-code">{{ formattedContent }}</pre>
      </div>

      <!-- 代码内容 -->
      <div v-else-if="detectedContent.type.type === 'code'" class="code-content">
        <div class="content-header">
          <span class="content-label">代码片段</span>
        </div>
        <pre class="formatted-code" v-html="highlightedContent" ref="codeElement"></pre>
      </div>

      <!-- URL 内容 -->
      <div v-else-if="detectedContent.type.type === 'url'" class="url-content">
        <div class="content-header">
          <span class="content-label">链接地址</span>
        </div>
        <div class="url-list">
          <div v-for="(url, index) in extractedUrls" :key="index" class="url-item">
            <i class="iconfont icon-link"></i>
            <a :href="url" target="_blank" rel="noopener noreferrer" class="url-link">{{ url }}</a>
          </div>
        </div>
      </div>

      <!-- 邮箱内容 -->
      <div v-else-if="detectedContent.type.type === 'email'" class="email-content">
        <div class="content-header">
          <span class="content-label">邮箱地址</span>
        </div>
        <div class="email-list">
          <div v-for="(email, index) in extractedEmails" :key="index" class="email-item">
            <i class="iconfont icon-mail"></i>
            <a :href="`mailto:${email}`" class="email-link">{{ email }}</a>
          </div>
        </div>
      </div>

      <!-- 默认文本内容 -->
      <div v-else class="text-content">
        <p class="text-preview">{{ currentDisplayContent }}</p>
      </div>
    </div>

    <!-- 展开控制 -->
    <div v-if="shouldShowExpand" class="expand-controls">
      <button class="expand-btn" @click.stop="toggleExpand" :disabled="isLoadingFullContent">
        <span v-if="isLoadingFullContent" class="loading-text">
          <span class="loading-spinner"></span>
          加载中...
        </span>
        <span v-else>
          {{ isExpanded ? '收起内容' : '展开内容' }}
          <i class="expand-icon" :class="{ 'expanded': isExpanded }"></i>
        </span>
      </button>
    </div>

    <!-- 粘性收起按钮 -->
    <div v-if="isExpanded" class="sticky-collapse" :class="{ 'visible': showStickyCollapse }">
      <button class="sticky-collapse-btn" @click.stop="toggleExpand">
        收起内容
        <i class="sticky-collapse-icon"></i>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, nextTick, watch } from 'vue';
import hljs from 'highlight.js/lib/core';
import { detectContentType, formatContent, getHighlightLanguage, type DetectedContent } from '../utils/contentDetector';
import { marked } from 'marked';

// 只引入必要的语言
import json from 'highlight.js/lib/languages/json';
import sql from 'highlight.js/lib/languages/sql';
import xml from 'highlight.js/lib/languages/xml'; // 用于HTML和XML
import javascript from 'highlight.js/lib/languages/javascript';

// 注册语言
hljs.registerLanguage('json', json);
hljs.registerLanguage('sql', sql);
hljs.registerLanguage('html', xml);
hljs.registerLanguage('xml', xml);
hljs.registerLanguage('javascript', javascript);

interface Props {
  content: string;
  showTypeIndicator?: boolean;
  maxHeight?: number;
  autoExpand?: boolean;
  isTruncated?: boolean;
  onLoadFullContent?: () => Promise<string>;
}

const props = withDefaults(defineProps<Props>(), {
  showTypeIndicator: false,
  maxHeight: 300,
  autoExpand: false,
  isTruncated: false
});

const emit = defineEmits<{
  (e: 'copy', content: string): void;
  (e: 'update:content', content: string): void;
}>();

// 响应式数据
const isExpanded = ref(props.autoExpand);
const showScrollbar = ref(false);
const shouldShowExpand = ref(false);
const shouldShowOverlay = ref<'none' | 'partial' | 'full'>('none');
const showStickyCollapse = ref(false);
const markdownViewMode = ref<'preview' | 'code'>('preview');
const contentArea = ref<HTMLElement | null>(null);
const codeElement = ref<HTMLElement | null>(null);

// 内容检测（基于当前显示内容）
const detectedContent = computed<DetectedContent>(() => {
  return detectContentType(currentDisplayContent.value);
});

// 格式化内容
const formattedContent = computed(() => {
  return formatContent(currentDisplayContent.value, detectedContent.value.type);
});

// 简化的语法高亮 - 性能优先
const highlightedContent = computed(() => {
  const content = formattedContent.value;

  // 内容太大时跳过高亮，避免性能问题
  if (content.length > 50 * 1024) { // 50KB以上不高亮
    return content;
  }

  try {
    // 使用 hljs 的自动检测，简单有效
    const result = hljs.highlightAuto(content, ['javascript', 'json', 'html', 'css', 'sql']);
    return result.value;
  } catch (error) {
    // 高亮失败就返回原内容，不影响功能
    return content;
  }
});

// Markdown 渲染（基于当前显示内容）
const renderedMarkdown = computed(() => {
  if (detectedContent.value.type.type === 'markdown') {
    try {
      return marked(currentDisplayContent.value, {
        breaks: true,
        gfm: true,
      });
    } catch (error) {
      console.warn('Markdown渲染失败:', error);
      return currentDisplayContent.value;
    }
  }
  return '';
});

// 提取URL（基于当前显示内容）
const extractedUrls = computed(() => {
  if (detectedContent.value.type.type === 'url') {
    const urlPattern = /https?:\/\/[^\s]+/gi;
    return currentDisplayContent.value.match(urlPattern) || [];
  }
  return [];
});

// 提取邮箱（基于当前显示内容）
const extractedEmails = computed(() => {
  if (detectedContent.value.type.type === 'email') {
    const emailPattern = /[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}/gi;
    return currentDisplayContent.value.match(emailPattern) || [];
  }
  return [];
});

// 获取类型图标
const getTypeIcon = (type: any) => {
  const iconMap: Record<string, string> = {
    json: 'icon-json',
    markdown: 'icon-text',
    code: 'icon-code',
    url: 'icon-link',
    email: 'icon-mail',
    text: 'icon-text'
  };
  return iconMap[type.type] || 'icon-text';
};

// 获取类型显示名称
const getTypeDisplayName = (type: any) => {
  const nameMap: Record<string, string> = {
    json: 'JSON',
    markdown: 'Markdown',
    code: '代码片段',
    url: 'URL',
    email: '邮箱',
    text: '文本'
  };
  return nameMap[type.type] || '文本';
};

// 复制内容
const copyContent = async () => {
  try {
    const contentToCopy = formattedContent.value;
    await navigator.clipboard.writeText(contentToCopy);
    emit('copy', contentToCopy);
  } catch (error) {
    console.error('复制失败:', error);
  }
};

// 当前显示内容（支持渐进加载）
const currentDisplayContent = ref(props.content);
const isLoadingFullContent = ref(false);

// 渐进式更新大内容，避免UI卡顿
const updateContentProgressively = async (content: string) => {
  if (content.length <= 200 * 1024) {
    // 小于200KB直接更新
    currentDisplayContent.value = content;
    return;
  }

  // 大内容分批更新，使用requestAnimationFrame确保不阻塞渲染
  const chunkSize = 50 * 1024; // 减少到50KB每块，更平滑
  let currentPos = 0;

  const updateChunk = () => {
    const nextPos = Math.min(currentPos + chunkSize, content.length);
    currentDisplayContent.value = content.substring(0, nextPos);
    currentPos = nextPos;

    if (currentPos < content.length) {
      // 使用requestAnimationFrame而不是setTimeout，更好的性能
      requestAnimationFrame(updateChunk);
    }
  };

  requestAnimationFrame(updateChunk);
};

// 切换展开状态
const toggleExpand = async () => {
  if (!isExpanded.value && props.isTruncated && props.onLoadFullContent) {
    // 展开截断内容时，先获取全量数据
    isLoadingFullContent.value = true;
    try {
      const fullContent = await props.onLoadFullContent();
      // 渐进式更新内容避免卡顿
      await updateContentProgressively(fullContent);
      emit('update:content', fullContent);
    } catch (error) {
      console.error('获取全量内容失败:', error);
      // 即使失败也要展开当前内容
    } finally {
      isLoadingFullContent.value = false;
    }
  }
  
  isExpanded.value = !isExpanded.value;
};

// 处理滚动
const handleScroll = () => {
  if (!contentArea.value) return;
  showStickyCollapse.value = contentArea.value.scrollTop > 100;
};

// 检查是否需要展开功能
const checkExpandNeeded = () => {
  if (!contentArea.value) return;
  
  const maxHeight = props.maxHeight;
  const actualHeight = contentArea.value.scrollHeight;
  
  shouldShowExpand.value = actualHeight > maxHeight;
  
  if (actualHeight > maxHeight * 1.5) {
    shouldShowOverlay.value = 'full';
  } else if (actualHeight > maxHeight) {
    shouldShowOverlay.value = 'partial';
  } else {
    shouldShowOverlay.value = 'none';
  }
};

// 监听展开状态变化
watch(isExpanded, (newVal) => {
  if (newVal) {
    nextTick(() => {
      if (contentArea.value) {
        contentArea.value.addEventListener('scroll', handleScroll);
        showScrollbar.value = contentArea.value.scrollHeight > contentArea.value.clientHeight;
      }
    });
  } else {
    if (contentArea.value) {
      contentArea.value.removeEventListener('scroll', handleScroll);
      showStickyCollapse.value = false;
    }
  }
});

// 监听内容变化
watch(() => props.content, (newContent) => {
  currentDisplayContent.value = newContent;
  nextTick(() => {
    checkExpandNeeded();
  });
});

// 组件挂载
onMounted(() => {
  currentDisplayContent.value = props.content;
  nextTick(() => {
    checkExpandNeeded();
  });
});

// 组件卸载
onBeforeUnmount(() => {
  if (contentArea.value) {
    contentArea.value.removeEventListener('scroll', handleScroll);
  }
});
</script>

<style scoped>
/* 引入自定义highlight.js样式 */
@import '../assets/styles/highlight.css';

.smart-content-display {
  width: 100%;
  position: relative;
}

/* 类型指示器 */
.content-type-indicator {
  margin-bottom: 8px;
}

.type-badge {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  border-radius: 6px;
  font-size: 11px;
  font-weight: 500;
  background: var(--type-bg, #f1f5f9);
  color: var(--type-color, #64748b);
  border: 1px solid var(--type-border, #e2e8f0);
}

.type-badge.type-json {
  background: #fef3c7;
  color: #d97706;
  border-color: #fbbf24;
}

.type-badge.type-code {
  background: #e6fffa;
  color: #319795;
  border-color: #81e6d9;
}

.type-badge.type-url {
  background: #f0fff4;
  color: #38a169;
  border-color: #9ae6b4;
}

.type-badge.type-email {
  background: #faf5ff;
  color: #805ad5;
  border-color: #c4b5fd;
}

.type-badge.type-markdown {
  background: #fef7ff;
  color: #a21caf;
  border-color: #d8b4fe;
}

.confidence-text {
  opacity: 0.7;
  font-size: 10px;
}

/* 内容显示区域 */
.content-display-area {
  position: relative;
  border-radius: 8px;
  border: 1px solid var(--border-color, #e2e8f0);
  background: var(--content-bg, #ffffff);
  overflow: hidden;
  transition: all 0.3s ease;
}

.content-display-area:not(.is-expanded) {
  max-height: 300px;
  overflow: hidden;
}

.content-display-area.is-expanded {
  max-height: 500px;
  overflow-y: auto;
}

.content-display-area.scroll-visible {
  scrollbar-width: thin;
  scrollbar-color: var(--scrollbar-thumb, #cbd5e1) var(--scrollbar-track, #f1f5f9);
}

.content-display-area.scroll-visible::-webkit-scrollbar {
  width: 6px;
}

.content-display-area.scroll-visible::-webkit-scrollbar-track {
  background: var(--scrollbar-track, #f1f5f9);
  border-radius: 3px;
}

.content-display-area.scroll-visible::-webkit-scrollbar-thumb {
  background: var(--scrollbar-thumb, #cbd5e1);
  border-radius: 3px;
}

/* 遮罩效果 */
.content-display-area:not(.is-expanded).has-overlay.overlay-full::after {
  content: '';
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 60px;
  background: linear-gradient(to top, var(--content-bg, #ffffff) 60%, transparent);
  pointer-events: none;
  z-index: 2;
}

.content-display-area:not(.is-expanded).has-overlay.overlay-partial::after {
  content: '';
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 30px;
  background: linear-gradient(to top, var(--content-bg, #ffffff) 30%, transparent);
  opacity: 0.7;
  pointer-events: none;
  z-index: 2;
}

/* 内容头部 */
.content-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  background: #f8fafc;
  border-bottom: 1px solid #e2e8f0;
}

.content-label {
  font-size: 12px;
  font-weight: 500;
  color: #64748b;
}

.markdown-controls {
  display: flex;
  align-items: center;
  gap: 6px;
}

.view-btn {
  padding: 4px 8px;
  border: 1px solid #e2e8f0;
  background: transparent;
  color: #64748b;
  cursor: pointer;
  font-size: 11px;
  border-radius: 4px;
  transition: all 0.2s ease;
}

.view-btn.active {
  background: #2c7a7b;
  color: #ffffff;
  border-color: #2c7a7b;
}

.copy-btn {
  padding: 4px 8px;
  border: none;
  background: transparent;
  color: #64748b;
  cursor: pointer;
  border-radius: 4px;
  font-size: 12px;
  transition: all 0.2s ease;
}

.copy-btn:hover {
  background: #e2e8f0;
  color: #1e293b;
}

/* 格式化代码 */
.formatted-code {
  margin: 0;
  padding: 12px;
  font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
  font-size: 13px;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
  background: transparent;
  overflow-x: auto;
  /* 允许文本选择 */
  -webkit-user-select: text;
  -moz-user-select: text;
  -ms-user-select: text;
  user-select: text;
  /* 额外的文本选择优化 */
  -webkit-touch-callout: text;
  -webkit-user-drag: none;
}

/* 确保格式化代码内的所有元素都支持文本选择 */
.formatted-code * {
  -webkit-user-select: text !important;
  -moz-user-select: text !important;
  -ms-user-select: text !important;
  user-select: text !important;
}

/* JSON、Markdown、代码 内容 */
.json-content,
.markdown-content,
.code-content {
  background: #f8fafc;
}

/* Markdown 预览 */
.markdown-preview-content {
  padding: 12px;
  background: #ffffff;
  max-height: 300px;
  overflow-y: auto;
  /* 允许文本选择 */
  -webkit-user-select: text;
  -moz-user-select: text;
  -ms-user-select: text;
  user-select: text;
}

/* Markdown 预览内容样式 */
.markdown-preview-content h1,
.markdown-preview-content h2,
.markdown-preview-content h3,
.markdown-preview-content h4,
.markdown-preview-content h5,
.markdown-preview-content h6 {
  margin-top: 16px;
  margin-bottom: 8px;
  font-weight: 600;
  color: #1f2937;
}

.markdown-preview-content h1 { font-size: 1.5em; }
.markdown-preview-content h2 { font-size: 1.3em; }
.markdown-preview-content h3 { font-size: 1.2em; }
.markdown-preview-content h4 { font-size: 1.1em; }

.markdown-preview-content p {
  margin-bottom: 8px;
  line-height: 1.6;
}

.markdown-preview-content ul,
.markdown-preview-content ol {
  margin-bottom: 8px;
  padding-left: 20px;
}

.markdown-preview-content li {
  margin-bottom: 4px;
}

.markdown-preview-content blockquote {
  margin: 8px 0;
  padding: 8px 12px;
  border-left: 4px solid #e5e7eb;
  background: #f9fafb;
  color: #6b7280;
}

.markdown-preview-content code {
  background: #f3f4f6;
  padding: 2px 4px;
  border-radius: 3px;
  font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
  font-size: 0.9em;
}

.markdown-preview-content pre {
  background: #f8fafc;
  padding: 12px;
  border-radius: 6px;
  overflow-x: auto;
  margin: 8px 0;
}

.markdown-preview-content pre code {
  background: none;
  padding: 0;
}

/* URL 和邮箱内容 */
.url-content,
.email-content {
  padding: 12px;
}

.url-list,
.email-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.url-item,
.email-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px;
  background: #f8fafc;
  border-radius: 6px;
  border: 1px solid #e2e8f0;
}

.url-link,
.email-link {
  color: #2563eb;
  text-decoration: none;
  word-break: break-all;
  font-size: 13px;
}

.url-link:hover,
.email-link:hover {
  text-decoration: underline;
}

/* 默认文本内容 */
.text-content {
  padding: 12px;
}

.text-preview {
  font-size: 14px;
  color: var(--text-primary, #2d3748);
  line-height: 1.7;
  white-space: pre-wrap;
  word-break: break-word;
  margin: 0;
  /* 允许文本选择 */
  -webkit-user-select: text;
  -moz-user-select: text;
  -ms-user-select: text;
  user-select: text;
}

/* 展开控制 */
.expand-controls {
  display: flex;
  justify-content: center;
  padding-top: 10px;
  opacity: 1;
  transition: opacity 0.3s ease;
}

.expand-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--text-secondary, #64748b);
  background: rgba(255, 255, 255, 0.9);
  border: 1px solid var(--border-color, #e2e8f0);
  border-radius: 16px;
  padding: 5px 14px;
  cursor: pointer;
  transition: all 0.2s ease;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.05);
  z-index: 2;
  position: relative;
}

.expand-btn:hover:not(:disabled) {
  color: var(--text-primary, #1e293b);
  background: var(--button-hover-bg, #f1f5f9);
  border-color: var(--border-hover-color, #cbd5e1);
  box-shadow: 0 3px 8px rgba(0, 0, 0, 0.08);
}

.expand-btn:disabled {
  opacity: 0.7;
  cursor: not-allowed;
}

.loading-text {
  display: flex;
  align-items: center;
  gap: 6px;
}

.loading-spinner {
  width: 12px;
  height: 12px;
  border: 2px solid rgba(0, 0, 0, 0.2);
  border-top-color: var(--text-secondary, #64748b);
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

.expand-icon {
  display: inline-block;
  width: 12px;
  height: 12px;
  background: currentColor;
  mask: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'%3E%3Cpath d='M7 10l5 5 5-5z'/%3E%3C/svg%3E") no-repeat center;
  -webkit-mask: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'%3E%3Cpath d='M7 10l5 5 5-5z'/%3E%3C/svg%3E") no-repeat center;
  mask-size: contain;
  -webkit-mask-size: contain;
  transition: transform 0.3s ease;
}

.expand-icon.expanded {
  transform: rotate(180deg);
}

/* 粘性收起按钮 */
.sticky-collapse {
  position: sticky;
  bottom: 15px;
  display: flex;
  justify-content: center;
  width: 100%;
  z-index: 10;
  pointer-events: none;
  opacity: 0;
  transform: translateY(10px);
  transition: opacity 0.3s ease, transform 0.3s ease;
  margin-top: -30px;
  padding-bottom: 10px;
}

.sticky-collapse.visible {
  opacity: 1;
  transform: translateY(0);
  pointer-events: auto;
}

.sticky-collapse-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: #fff;
  background: rgba(44, 122, 123, 0.85);
  border: none;
  border-radius: 18px;
  padding: 6px 18px;
  cursor: pointer;
  transition: all 0.2s ease;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  backdrop-filter: blur(4px);
}

.sticky-collapse-btn:hover {
  background: var(--primary-color, #2c7a7b);
  box-shadow: 0 5px 15px rgba(0, 0, 0, 0.2);
}

.sticky-collapse-icon {
  display: inline-block;
  width: 12px;
  height: 12px;
  background: currentColor;
  mask: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'%3E%3Cpath d='M7 14l5-5 5 5z'/%3E%3C/svg%3E") no-repeat center;
  -webkit-mask: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'%3E%3Cpath d='M7 14l5-5 5 5z'/%3E%3C/svg%3E") no-repeat center;
  mask-size: contain;
  -webkit-mask-size: contain;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .formatted-code {
    font-size: 12px;
  }
  
  .content-header {
    padding: 6px 10px;
  }
  
  .view-btn,
  .copy-btn {
    font-size: 10px;
    padding: 3px 6px;
  }
}
</style> 