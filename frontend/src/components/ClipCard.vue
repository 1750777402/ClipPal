<template>
    <div class="clip-card" :class="{ 'clip-card-hover': !isMobile, 'is-pinned': record.pinned_flag }"
        @click="handleCardClick">
        <div class="card-header">
            <div class="card-type">
                <i class="iconfont" :class="getTypeIcon" :title="getTypeTitle"></i>
                <span class="type-text">{{ getTypeTitle }}
                    <template v-if="record.type === 'File'">
                        <span class="tip-icon-wrapper" @mouseenter="showTip = true" @mouseleave="showTip = false">
                            <i class="iconfont icon-tishi"></i>
                            <span v-if="showTip" class="tip-pop">该条目为文件类型，复制时会尝试复制所有文件路径，若有源文件丢失将提示失败</span>
                        </span>
                    </template>
                </span>
            </div>
            <div class="card-meta">
                <span class="time-text">{{ formatTime(record.created) }}</span>
                <div class="card-actions">
                    <button class="action-btn pin-btn" :class="{ 'is-pinned': record.pinned_flag }"
                        @click.stop="handlePin" :title="record.pinned_flag ? '取消置顶' : '置顶'">
                        <i class="iconfont" :class="record.pinned_flag ? 'icon-dingzhu' : 'icon-weizhiding'"></i>
                    </button>
                    <button class="action-btn" @click.stop="handleCardClick" title="复制">
                        <i class="iconfont icon-copy"></i>
                    </button>
                    <button class="action-btn" @click.stop="handleDelete" title="删除">
                        <i class="iconfont icon-delete"></i>
                    </button>
                    <button v-if="record.type === 'Image'" class="action-btn" @click.stop="handleSaveAs" title="另存为">
                        <i class="iconfont icon-lingcunwei"></i>
                    </button>
                </div>
            </div>
        </div>

        <div class="card-content">
            <!-- 文本类型 -->
            <template v-if="record.type === 'Text'">
                <div class="text-content-container">
                    <div class="text-content" :class="{
                        'is-expanded': isExpanded,
                        'has-overlay': shouldShowOverlay !== 'none',
                        'overlay-partial': shouldShowOverlay === 'partial',
                        'overlay-full': shouldShowOverlay === 'full',
                        'scroll-visible': showScrollbar
                    }" ref="textContent">
                        <p class="text-preview" :title="!isExpanded && shouldShowExpand ? record.content : ''"
                            ref="textPreview">
                            {{ record.content }}
                        </p>
                    </div>
                    <div v-if="shouldShowExpand" class="expand-controls">
                        <button class="expand-btn" @click.stop="toggleExpand">
                            {{ isExpanded ? '收起内容' : '展开内容' }}
                            <i class="expand-icon" :class="{ 'expanded': isExpanded }"></i>
                        </button>
                    </div>
                    <div v-if="isExpanded" class="sticky-collapse" :class="{ 'visible': showStickyCollapse }">
                        <button class="sticky-collapse-btn" @click.stop="toggleExpand">
                            收起内容
                            <i class="sticky-collapse-icon"></i>
                        </button>
                    </div>
                </div>
            </template>

            <!-- 图片类型 -->
            <template v-else-if="record.type === 'Image'">
                <div class="image-container">
                    <img v-if="record.content" :src="record.content" class="image-preview" @load="handleImageLoad"
                        @error="handleImageError" @click.stop="showImagePreview = true" />
                    <div v-if="!isImageLoaded" class="image-placeholder">
                        <div class="placeholder-spinner"></div>
                        <span class="loading-text">加载中...</span>
                    </div>
                </div>
            </template>

            <!-- 文件类型 -->
            <template v-else-if="record.type === 'File'">
                <div class="file-content">
                    <div class="file-list">
                        <div v-for="(file, index) in fileList" :key="index" class="file-item">
                            <div class="file-icon-wrapper">
                                <i class="iconfont icon-file"></i>
                            </div>
                            <div class="file-info">
                                <span class="file-name" :title="file.path">{{ getFileName(file.path) }}</span>
                                <span class="file-meta">
                                    {{ formatFileSize(file.size) }} · {{ file.type || getFileType(file.path) }}
                                </span>
                            </div>
                        </div>
                    </div>
                    <div class="file-count" v-if="fileList.length > 1">
                        共 {{ fileList.length }} 个文件
                    </div>
                </div>
            </template>

            <!-- JSON内容 -->
            <template v-else-if="record.type === 'JSON'">
                <div class="json-content">
                    <div class="content-icon">
                        <i class="iconfont icon-json"></i>
                    </div>
                    <pre class="json-preview">{{ formatJSON(record.content) }}</pre>
                </div>
            </template>

            <!-- 代码内容 -->
            <template v-else-if="record.type === 'Code'">
                <div class="code-content">
                    <div class="content-icon">
                        <i class="iconfont icon-code"></i>
                    </div>
                    <pre class="code-preview">{{ record.content }}</pre>
                </div>
            </template>

            <!-- 默认内容 -->
            <template v-else>
                <div class="default-content">
                    <div class="content-icon">
                        <i class="iconfont icon-default"></i>
                    </div>
                    <p class="text-preview">{{ record.content }}</p>
                </div>
            </template>
        </div>
    </div>

    <!-- 图片预览组件 -->
    <vue-easy-lightbox v-if="record.type === 'Image' && record.content" :visible="showImagePreview"
        :imgs="[{ src: record.content }]" :index="0" @hide="showImagePreview = false" />

    <template v-if="showConfirm">
        <div class="confirm-mask" @click.self="cancelDelete">
            <div class="confirm-dialog">
                <div class="confirm-title">删除确认</div>
                <div class="confirm-content">确定要删除该条记录吗？删除后无法恢复</div>
                <div class="confirm-actions">
                    <button class="confirm-btn confirm-cancel" @click="cancelDelete">取消</button>
                    <button class="confirm-btn confirm-ok" @click="confirmDelete">确定</button>
                </div>
            </div>
        </div>
    </template>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, nextTick, watch, inject } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { formatDistanceToNow } from 'date-fns';
import { zhCN } from 'date-fns/locale';
import VueEasyLightbox from 'vue-easy-lightbox';

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
}

interface FileInfo {
    path: string;
    size: number;
    type?: string;
}

const props = defineProps<{
    record: ClipRecord;
    isMobile: boolean;
}>();

const emit = defineEmits<{
    (e: 'click', record: ClipRecord): void;
    (e: 'copy', record: ClipRecord): void;
    (e: 'delete', record: ClipRecord): void;
    (e: 'pin', record: ClipRecord): void;
}>();

const isExpanded = ref(false);
const isImageLoaded = ref(false);
const showImagePreview = ref(false);
const textContent = ref<HTMLElement | null>(null);
const textPreview = ref<HTMLElement | null>(null);
const showStickyCollapse = ref(false);
const showScrollbar = ref(false);
const shouldShowExpand = ref(false);
// 修正为字符串类型，支持三种状态
const shouldShowOverlay = ref<'none' | 'partial' | 'full'>('none');
const showTip = ref(false);
const showConfirm = ref(false);

const LINE_HEIGHT = 24; // 根据实际行高设置
const MAX_LINES_FOR_FULL = 8; // 最多显示5行完整内容
const MAX_LINES_FOR_PREVIEW = 8; // 超过3行显示展开按钮

const showMessageBar = inject('showMessageBar') as (msg: string, type?: 'success' | 'error') => void;

const handleCardClick = async () => {
    try {
        await invoke('copy_clip_record', { param: { record_id: props.record.id } });
        emit('click', props.record);
    } catch (err: any) {
        if (showMessageBar) {
            showMessageBar(err?.toString() || '复制失败', 'error');
        } else {
            alert(err?.toString() || '复制失败');
        }
    }
};

const formatTime = (timestamp: number) => {
    return formatDistanceToNow(new Date(timestamp), {
        addSuffix: true,
        locale: zhCN
    });
};

const formatFileSize = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
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

const getTypeIcon = computed(() => {
    switch (props.record.type) {
        case 'Text':
            return 'icon-text';
        case 'Image':
            return 'icon-image';
        case 'File':
            return 'icon-file';
        case 'JSON':
            return 'icon-json';
        case 'Code':
            return 'icon-code';
        default:
            return 'icon-default';
    }
});

const getTypeTitle = computed(() => {
    switch (props.record.type) {
        case 'Text':
            return '文本';
        case 'Image':
            return '图片';
        case 'File':
            return '文件';
        case 'JSON':
            return 'JSON数据';
        case 'Code':
            return '代码片段';
        default:
            return '其他内容';
    }
});

const fileList = computed(() => {
    if (props.record.type === 'File') {
        try {
            return props.record.file_info || JSON.parse(props.record.content);
        } catch {
            return [];
        }
    }
    return [];
});

const handleSaveAs = async () => {
    let param_obj = { record_id: props.record.id };
    await invoke('image_save_as', { param: param_obj });
};

const handleDelete = async () => {
    showConfirm.value = true;
};

const confirmDelete = async () => {
    let param_obj = { record_id: props.record.id };
    await invoke('del_record', { param: param_obj });
    emit('delete', props.record);
    showConfirm.value = false;
};

const cancelDelete = () => {
    showConfirm.value = false;
};

const handlePin = async () => {
    let param_obj = { record_id: props.record.id, pinned_flag: !props.record.pinned_flag ? 1 : 0 };
    await invoke('set_pinned', { param: param_obj });
    emit('pin', props.record);
};

const toggleExpand = (event: Event) => {
    event.stopPropagation();
    isExpanded.value = !isExpanded.value;
};

const handleImageLoad = () => {
    isImageLoaded.value = true;
};

const handleImageError = () => {
    isImageLoaded.value = false;
};

const getFileName = (filePath: string) => {
    return filePath.split(/[\\/]/).pop() || filePath;
};

const handleTextScroll = () => {
    if (!textContent.value) return;
    showStickyCollapse.value = textContent.value.scrollTop > 100;
};

// 计算文本行数
const calculateTextLines = () => {
    if (!textPreview.value) return 0;

    // 获取实际高度
    const height = textPreview.value.clientHeight;
    // 计算行数
    const lines = Math.round(height / LINE_HEIGHT);

    return lines;
};

// 检查是否需要显示展开功能
const checkExpandNeeded = () => {
    const lines = calculateTextLines();

    // 计算是否需要显示展开按钮
    shouldShowExpand.value = lines > MAX_LINES_FOR_PREVIEW;

    // 计算遮罩类型
    if (lines > MAX_LINES_FOR_FULL) {
        shouldShowOverlay.value = 'full';
    } else if (lines > MAX_LINES_FOR_PREVIEW) {
        shouldShowOverlay.value = 'partial';
    } else {
        shouldShowOverlay.value = 'none';
    }
};

watch(isExpanded, (newVal) => {
    if (newVal) {
        nextTick(() => {
            if (textContent.value) {
                textContent.value.addEventListener('scroll', handleTextScroll);
                showScrollbar.value = textContent.value.scrollHeight > textContent.value.clientHeight;
            }
        });
    } else {
        if (textContent.value) {
            textContent.value.removeEventListener('scroll', handleTextScroll);
            showStickyCollapse.value = false;
        }
    }
});

onMounted(() => {
    if (props.record.type === 'Image' && props.record.content) {
        const img = new Image();
        img.src = props.record.content;
        img.onload = handleImageLoad;
        img.onerror = handleImageError;
    }

    // 初始检查文本行数
    nextTick(() => {
        checkExpandNeeded();

        // 添加resize监听器
        const resizeObserver = new ResizeObserver(() => {
            checkExpandNeeded();
        });

        if (textPreview.value) {
            resizeObserver.observe(textPreview.value);
        }

        // 组件卸载时断开监听
        onBeforeUnmount(() => {
            if (textPreview.value) {
                resizeObserver.unobserve(textPreview.value);
            }
        });
    });
});

onBeforeUnmount(() => {
    if (textContent.value) {
        textContent.value.removeEventListener('scroll', handleTextScroll);
    }
});
</script>

<style scoped>
.clip-card {
    background: var(--card-bg, #ffffff);
    border-radius: 12px;
    box-shadow: 0 3px 10px rgba(0, 0, 0, 0.06);
    margin: 0 20px 16px 20px;
    transition: all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
    cursor: pointer;
    border: 1px solid var(--border-color, #edf2f7);
    position: relative;
    overflow: hidden;
}

.clip-card-hover:hover {
    box-shadow: 0 6px 16px rgba(0, 0, 0, 0.1);
    border-color: var(--border-hover-color, #e2e8f0);
    transform: translateY(-3px);
}

.clip-card.is-pinned {
    background: var(--pinned-bg, #f8fafc);
    border-left: 4px solid var(--primary-color, #2c7a7b);
    box-shadow: 0 4px 12px rgba(44, 122, 123, 0.12);
}

.card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-color, #e2e8f0);
    background: var(--header-bg, #f8fafc);
    position: relative;
}

.card-type {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-secondary, #64748b);
    font-size: 13px;
    font-weight: 500;
}

.type-text {
    font-weight: 500;
}

.card-meta {
    display: flex;
    align-items: center;
    gap: 16px;
}

.time-text {
    font-size: 12px;
    color: var(--text-tertiary, #94a3b8);
    white-space: nowrap;
    transition: color 0.2s ease;
    font-weight: 400;
}

.card-actions {
    display: flex;
    align-items: center;
    gap: 6px;
}

.action-btn {
    width: 28px;
    height: 28px;
    border-radius: 8px;
    border: none;
    background: transparent;
    color: var(--text-secondary, #a0aec0);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s ease;
    padding: 0;
}

.action-btn:hover {
    background: var(--button-hover-bg, #f1f5f9);
    color: var(--text-primary, #1e293b);
    transform: translateY(-1px);
}

.action-btn:active {
    transform: translateY(0);
}

.action-btn.pin-btn.is-pinned {
    color: var(--primary-color, #2c7a7b);
    background: rgba(44, 122, 123, 0.1);
}

.card-content {
    padding: 16px;
    background: var(--card-bg, #ffffff);
    position: relative;
}

/* 文本内容样式 */
.text-content-container {
    position: relative;
}

.text-content {
    overflow: hidden;
    transition:
        max-height 0.4s cubic-bezier(0.4, 0, 0.2, 1),
        border-color 0.3s ease,
        padding 0.3s ease;
    background: var(--card-bg, #ffffff);
    border-radius: 6px;
    position: relative;
    border: 1px solid transparent;
    padding: 0;

    /* 默认高度，3行以内 */
    max-height: none;
}

.text-content.is-expanded {
    max-height: 400px;
    overflow-y: auto;
    border-color: var(--border-color, #e2e8f0);
    padding: 12px;
}

/* 平滑滚动条显示 */
.text-content.scroll-visible::-webkit-scrollbar {
    width: 6px;
}

.text-content.scroll-visible::-webkit-scrollbar-track {
    background: var(--scrollbar-track, #f1f5f9);
    border-radius: 3px;
}

.text-content.scroll-visible::-webkit-scrollbar-thumb {
    background: var(--scrollbar-thumb, #cbd5e1);
    border-radius: 3px;
}

.text-content.scroll-visible::-webkit-scrollbar-thumb:hover {
    background: var(--scrollbar-thumb-hover, #94a3b8);
}

/* 遮罩效果 */
.text-content:not(.is-expanded).has-overlay.overlay-full::after {
    content: '';
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    height: 40px;
    background: linear-gradient(to top, var(--card-bg, #ffffff) 60%, transparent);
    transition: opacity 0.3s ease;
}

.text-content:not(.is-expanded).has-overlay.overlay-partial::after {
    content: '';
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    height: 20px;
    background: linear-gradient(to top, var(--card-bg, #ffffff) 30%, transparent);
    opacity: 0.7;
    transition: opacity 0.3s ease;
}

.text-preview {
    font-size: 14px;
    color: var(--text-primary, #2d3748);
    line-height: 1.7;
    white-space: pre-wrap;
    word-break: break-word;
    margin: 0;
    padding: 0;
    transition: all 0.3s ease;
}

/* 4-5行内容样式 */
.text-content.overlay-partial:not(.is-expanded) {
    max-height: calc(5 * 24px);
    /* 5行高度 */
}

/* 超过5行内容样式 */
.text-content.overlay-full:not(.is-expanded) {
    max-height: calc(3 * 24px);
    /* 3行高度 */
}

/* 展开控制区域 */
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

.expand-btn:hover {
    color: var(--text-primary, #1e293b);
    background: var(--button-hover-bg, #f1f5f9);
    border-color: var(--border-hover-color, #cbd5e1);
    box-shadow: 0 3px 8px rgba(0, 0, 0, 0.08);
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
    transition:
        opacity 0.3s ease,
        transform 0.3s ease;
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

/* 图片内容样式 */
.image-container {
    position: relative;
    width: 100%;
    height: 200px;
    border-radius: 8px;
    overflow: hidden;
    background: var(--image-bg, #f8fafc);
    display: flex;
    align-items: center;
    justify-content: center;
}

.image-preview {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    cursor: zoom-in;
    transition: transform 0.3s ease;
    border-radius: 4px;
}

.image-preview:hover {
    transform: scale(1.03);
}

.image-placeholder {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    gap: 12px;
}

.placeholder-spinner {
    width: 32px;
    height: 32px;
    border: 3px solid rgba(100, 116, 139, 0.1);
    border-top-color: var(--primary-color, #2c7a7b);
    border-radius: 50%;
    animation: spin 1s linear infinite;
}

.loading-text {
    font-size: 12px;
    color: var(--text-secondary, #64748b);
}

/* 文件内容样式 */
.file-content {
    background: var(--card-bg, #fff);
    border-radius: 8px;
}

.file-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-height: 180px;
    overflow-y: auto;
    padding: 4px;
    padding-right: 8px;
}

.file-list::-webkit-scrollbar {
    width: 5px;
}

.file-list::-webkit-scrollbar-track {
    background: var(--scrollbar-track, #f1f5f9);
    border-radius: 4px;
}

.file-list::-webkit-scrollbar-thumb {
    background: var(--scrollbar-thumb, #cbd5e1);
    border-radius: 4px;
    transition: background 0.2s ease;
}

.file-list::-webkit-scrollbar-thumb:hover {
    background: var(--scrollbar-thumb-hover, #94a3b8);
}

.file-item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    background: var(--item-bg, #f8fafc);
    border-radius: 8px;
    transition: all 0.2s ease;
    border: 1px solid var(--border-color, #e2e8f0);
}

.file-item:hover {
    background: var(--item-hover-bg, #f1f5f9);
    border-color: var(--border-hover-color, #cbd5e1);
    transform: translateY(-2px);
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.05);
}

.file-icon-wrapper {
    width: 36px;
    height: 36px;
    border-radius: 8px;
    background: rgba(44, 122, 123, 0.1);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
}

.file-icon-wrapper i {
    color: var(--primary-color, #2c7a7b);
}

.file-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
}

.file-name {
    font-size: 13px;
    color: var(--text-primary, #2d3748);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: 500;
}

.file-meta {
    font-size: 11px;
    color: var(--text-secondary, #64748b);
}

.file-count {
    margin-top: 10px;
    font-size: 11px;
    color: var(--text-secondary, #64748b);
    text-align: right;
    padding-right: 6px;
}

/* JSON和代码内容样式 */
.json-content,
.code-content,
.default-content {
    display: flex;
    gap: 12px;
    background: var(--item-bg, #f8fafc);
    border-radius: 8px;
    padding: 14px;
    border: 1px solid var(--border-color, #e2e8f0);
    transition: all 0.3s ease;
}

.content-icon {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    background: rgba(44, 122, 123, 0.1);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
}

.content-icon i {
    color: var(--primary-color, #2c7a7b);
}

.json-preview,
.code-preview {
    flex: 1;
    font-size: 13px;
    color: var(--text-primary, #2d3748);
    white-space: pre-wrap;
    word-break: break-word;
    margin: 0;
    line-height: 1.5;
    overflow-x: auto;
    max-height: 180px;
    transition: all 0.3s ease;
}

.tip-icon-wrapper {
    display: inline-block;
    position: relative;
    margin-left: 4px;
    cursor: pointer;
}

.tip-icon-wrapper .iconfont {
    font-size: 16px;
    color: #2c7a7b;
    vertical-align: middle;
}

.tip-pop {
    position: absolute;
    left: 50%;
    top: 130%;
    transform: translateX(-50%);
    background: #fffbe6;
    color: #222;
    border: 1px solid #ffe58f;
    border-radius: 8px;
    padding: 8px 18px;
    font-size: 13px;
    line-height: 1.6;
    max-width: 220px;
    min-width: 120px;
    white-space: normal;
    box-shadow: 0 6px 24px rgba(44, 122, 123, 0.18);
    z-index: 20;
    margin-top: 8px;
    text-align: left;
    word-break: break-all;
}

.tip-pop::before {
    content: '';
    position: absolute;
    top: -8px;
    left: 50%;
    transform: translateX(-50%);
    border-width: 0 8px 8px 8px;
    border-style: solid;
    border-color: transparent transparent #fffbe6 transparent;
    filter: drop-shadow(0 -1px 0 #ffe58f);
}

.confirm-mask {
    position: fixed;
    left: 0;
    top: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.18);
    z-index: 10000;
    display: flex;
    align-items: center;
    justify-content: center;
}

.confirm-dialog {
    background: #fff;
    border-radius: 12px;
    box-shadow: 0 8px 32px rgba(44, 122, 123, 0.18);
    padding: 28px 32px 20px 32px;
    min-width: 260px;
    max-width: 90vw;
    text-align: center;
    animation: popin 0.18s cubic-bezier(.4, 1.6, .6, 1) both;
}

@keyframes popin {
    0% {
        transform: scale(0.8);
        opacity: 0;
    }

    100% {
        transform: scale(1);
        opacity: 1;
    }
}

.confirm-title {
    font-size: 18px;
    font-weight: 600;
    color: #2c7a7b;
    margin-bottom: 12px;
}

.confirm-content {
    font-size: 15px;
    color: #333;
    margin-bottom: 22px;
}

.confirm-actions {
    display: flex;
    justify-content: center;
    gap: 18px;
}

.confirm-btn {
    min-width: 68px;
    padding: 7px 0;
    border-radius: 8px;
    border: none;
    font-size: 15px;
    cursor: pointer;
    transition: background 0.2s;
}

.confirm-cancel {
    background: #f5f7fa;
    color: #2c7a7b;
    border: 1px solid #e2e8f0;
}

.confirm-ok {
    background: var(--primary-color, #2c7a7b);
    color: #fff;
    border: none;
}

.confirm-btn:active {
    opacity: 0.85;
}
</style>