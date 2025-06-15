<template>
    <div class="clip-card" :class="{ 'clip-card-hover': !isMobile }" @click="handleCardClick">
        <div class="clip-content">
            <!-- 文本内容 -->
            <template v-if="record.type === 'Text'">
                <div class="text-content">
                    <div class="content-icon">
                        <i class="icon-text"></i>
                    </div>
                    <p class="text-preview" :class="{ 'mask-visible': shouldShowMask(record.content) }"
                        :title="record.content">
                        {{ record.content }}
                    </p>
                </div>
            </template>

            <!-- 图片内容 -->
            <template v-else-if="record.type === 'Image'">
                <div class="image-content">
                    <div class="content-icon">
                        <i class="icon-image"></i>
                    </div>
                    <div ref="imageContainer" class="image-container">
                        <InnerImageZoom v-if="isVisible" :src="record.content" :zoomSrc="record.content"
                            :zoomScale="0.7" moveType="pan" zoomType="hover" :fadeDuration="300" class="image-preview"
                            loading="lazy" />
                        <div v-else class="image-placeholder">
                            <div class="placeholder-spinner"></div>
                        </div>
                    </div>
                </div>
            </template>

            <!-- 文件内容 -->
            <template v-else-if="record.type === 'File'">
                <div class="file-content">
                    <div class="content-icon">
                        <i class="icon-file"></i>
                    </div>
                    <div class="file-preview">
                        <div class="file-name" :title="record.content">{{ record.content }}</div>
                        <div class="file-info">
                            <span class="file-size">{{ formatFileSize(record.fileSize) }}</span>
                            <span class="file-type">{{ getFileType(record.content) }}</span>
                        </div>
                    </div>
                </div>
            </template>

            <!-- JSON内容 -->
            <template v-else-if="record.type === 'JSON'">
                <div class="json-content">
                    <div class="content-icon">
                        <i class="icon-json"></i>
                    </div>
                    <pre class="json-preview">{{ formatJSON(record.content) }}</pre>
                </div>
            </template>

            <!-- 代码内容 -->
            <template v-else-if="record.type === 'Code'">
                <div class="code-content">
                    <div class="content-icon">
                        <i class="icon-code"></i>
                    </div>
                    <pre class="code-preview">{{ record.content }}</pre>
                </div>
            </template>

            <!-- 默认内容 -->
            <template v-else>
                <div class="default-content">
                    <div class="content-icon">
                        <i class="icon-default"></i>
                    </div>
                    <p class="text-preview">{{ record.content }}</p>
                </div>
            </template>
        </div>

        <div class="clip-meta">
            <span class="clip-time">{{ formatTime(record.created) }}</span>
        </div>
    </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import InnerImageZoom from 'vue-inner-image-zoom';
import dayjs from 'dayjs';
import relativeTime from 'dayjs/plugin/relativeTime';
import 'dayjs/locale/zh-cn';

dayjs.extend(relativeTime);
dayjs.locale('zh-cn');

interface ClipRecord {
    id: string;
    type: string;
    content: string;
    created: number;
    user_id: number;
    os_type: string;
    fileSize?: number;
}

const props = defineProps<{
    record: ClipRecord;
    isMobile: boolean;
}>();

const emit = defineEmits<{
    (e: 'click', record: ClipRecord): void;
}>();

const imageContainer = ref<HTMLElement | null>(null);
const isVisible = ref(false);
let observer: IntersectionObserver | null = null;

const handleCardClick = async () => {
    await invoke('copy_clip_record', { param: { record_id: props.record.id } });
    emit('click', props.record);
};

const shouldShowMask = (text: string) => {
    return text.split('\n').length > 3 || text.length > 100;
};

const formatTime = (timestamp: number) => {
    return dayjs(timestamp).fromNow();
};

const formatFileSize = (size?: number) => {
    if (!size) return '';
    const units = ['B', 'KB', 'MB', 'GB'];
    let value = size;
    let unitIndex = 0;
    while (value >= 1024 && unitIndex < units.length - 1) {
        value /= 1024;
        unitIndex++;
    }
    return `${value.toFixed(1)} ${units[unitIndex]}`;
};

const getFileType = (filename: string) => {
    const ext = filename.split('.').pop()?.toLowerCase() || '';
    const typeMap: Record<string, string> = {
        'pdf': 'PDF文档',
        'doc': 'Word文档',
        'docx': 'Word文档',
        'xls': 'Excel表格',
        'xlsx': 'Excel表格',
        'ppt': 'PPT演示',
        'pptx': 'PPT演示',
        'txt': '文本文件',
        'zip': '压缩文件',
        'rar': '压缩文件',
        '7z': '压缩文件',
        'jpg': '图片',
        'jpeg': '图片',
        'png': '图片',
        'gif': '图片',
        'mp3': '音频',
        'mp4': '视频',
        'mov': '视频',
        'avi': '视频'
    };
    return typeMap[ext] || '文件';
};

const formatJSON = (json: string) => {
    try {
        const parsed = JSON.parse(json);
        return JSON.stringify(parsed, null, 2);
    } catch {
        return json;
    }
};

onMounted(() => {
    if ('IntersectionObserver' in window && imageContainer.value) {
        observer = new IntersectionObserver(
            (entries) => {
                entries.forEach(entry => {
                    if (entry.isIntersecting) {
                        isVisible.value = true;
                        observer?.disconnect();
                    }
                });
            },
            { threshold: 0.1 }
        );
        observer.observe(imageContainer.value);
    } else {
        isVisible.value = true;
    }
});

onBeforeUnmount(() => {
    observer?.disconnect();
});
</script>

<style scoped>
.clip-card {
    background: var(--card-bg, #ffffff);
    border-radius: 16px;
    box-shadow: 0 2px 10px rgba(38, 50, 56, 0.07);
    padding: 16px;
    margin: 0 20px 16px 20px;
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    cursor: pointer;
    will-change: transform, box-shadow;
}

.clip-card-hover:hover {
    box-shadow: 0 8px 24px rgba(38, 50, 56, 0.15);
    transform: translateY(-2px);
}

.clip-content {
    flex: 1;
    overflow: hidden;
    padding-right: 12px;
    min-width: 0;
}

.content-icon {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    background: var(--icon-bg, #f0f4f8);
    display: flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 8px;
}

.content-icon i {
    font-size: 18px;
    color: var(--icon-color, #2c7a7b);
}

.text-content,
.image-content,
.file-content,
.json-content,
.code-content,
.default-content {
    display: flex;
    flex-direction: column;
}

.text-preview {
    font-size: 15px;
    color: var(--text-primary, #2d3748);
    line-height: 1.6;
    max-height: 4.8em;
    overflow: hidden;
    position: relative;
    white-space: normal;
    word-break: break-word;
    padding-right: 6px;
    transition: max-height 0.3s ease;
}

.text-preview::after {
    content: "";
    position: absolute;
    bottom: 0;
    left: 0;
    width: 100%;
    height: 2em;
    background: linear-gradient(to bottom, rgba(255, 255, 255, 0), var(--card-bg, #fff) 90%);
    opacity: 0;
    transition: opacity 0.3s ease;
    pointer-events: none;
}

.text-preview.mask-visible::after {
    opacity: 1;
}

.image-container {
    position: relative;
    width: 180px;
    height: 120px;
    border-radius: 12px;
    overflow: hidden;
    background: var(--image-bg, #f0f4f8);
}

.image-placeholder {
    width: 100%;
    height: 100%;
    display: flex;
    justify-content: center;
    align-items: center;
    background: var(--placeholder-bg, #f0f4f8);
}

.placeholder-spinner {
    width: 24px;
    height: 24px;
    border: 2px solid var(--spinner-border, #e0f2f1);
    border-top-color: var(--spinner-color, #2c7a7b);
    border-radius: 50%;
    animation: spin 1s linear infinite;
}

.image-preview {
    width: 100%;
    height: 100%;
    object-fit: contain;
    border-radius: 12px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
    transition: transform 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    cursor: pointer;
    margin: 0;
    will-change: transform;
}

.image-preview:hover {
    transform: scale(1.05);
}

.file-preview {
    display: flex;
    flex-direction: column;
    padding: 12px;
    font-size: 15px;
    color: var(--text-primary, #2d3748);
    background-color: var(--file-bg, #f0f4f8);
    border-radius: 12px;
    box-shadow: 0 1px 6px rgba(50, 60, 70, 0.1);
    word-break: break-word;
    max-width: 300px;
    cursor: default;
    user-select: text;
    line-height: 1.4;
    transition: background-color 0.3s ease;
}

.file-name {
    white-space: normal;
    overflow-wrap: break-word;
    word-break: break-word;
    margin-bottom: 4px;
}

.file-info {
    display: flex;
    gap: 8px;
    font-size: 13px;
    color: var(--text-secondary, #718096);
    user-select: none;
}

.json-preview,
.code-preview {
    background: var(--code-bg, #f8fafc);
    padding: 12px;
    border-radius: 12px;
    font-family: 'Fira Code', monospace;
    font-size: 14px;
    line-height: 1.5;
    overflow-x: auto;
    white-space: pre-wrap;
    word-break: break-word;
    color: var(--code-text, #2d3748);
    max-height: 200px;
    overflow-y: auto;
}

.clip-meta {
    font-size: 13px;
    color: var(--text-secondary, #718096);
    white-space: nowrap;
    margin-left: 12px;
}

@keyframes spin {
    to {
        transform: rotate(360deg);
    }
}

/* 图标样式 */
.icon-text::before {
    content: "📝";
}

.icon-image::before {
    content: "🖼️";
}

.icon-file::before {
    content: "📁";
}

.icon-json::before {
    content: "📋";
}

.icon-code::before {
    content: "💻";
}

.icon-default::before {
    content: "📌";
}
</style>