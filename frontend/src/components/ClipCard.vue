<template>
    <div class="clip-card" :class="{'clip-card-hover': !isMobile, 'is-pinned': record.pinned_flag}"
        @dblclick="handleCardDoubleClick">
        <div class="card-header">
            <div class="card-left">
                <div class="card-type">
                    <i class="iconfont" :class="getTypeIcon" :title="getTypeTitle"></i>
                    <span class="type-text">{{ getTypeTitle }}
                        <template v-if="record.type === 'File' && fileList.length > 1">
                            <span class="tip-icon-wrapper" @mouseenter="showTip = true" @mouseleave="showTip = false">
                                <i class="iconfont icon-tishi"></i>
                                <span v-if="showTip" class="tip-pop">该条目为多文件类型，双击复制全部文件，或点击单个文件的"仅复制此文件"按钮。若有源文件丢失将提示失败</span>
                            </span>
                        </template>
                    </span>
                    <div v-if="cloudSyncEnabled && record.sync_flag !== undefined" class="sync-dot">
                        <span v-if="record.sync_flag === 0" class="sync-unsynced" title="未同步"></span>
                        <span v-else-if="record.sync_flag === 1" class="sync-syncing" :title="getSyncingTitle">
                            <i class="iconfont icon-loading sync-loading"></i>
                        </span>
                        <span v-else-if="record.sync_flag === 2" class="sync-synced" title="已同步"></span>
                        <span v-else-if="record.sync_flag === 3" class="sync-skipped" title="不支持同步">
                            <i class="iconfont icon-tishi"></i>
                        </span>
                    </div>
                </div>
                <span class="time-text">{{ formatTime(record.created) }}</span>
            </div>
            <div class="card-actions" @click.stop @dblclick.stop>
                <button class="action-btn pin-btn" :class="{ 'is-pinned': record.pinned_flag }"
                    @click.stop="handlePin" :title="record.pinned_flag ? '取消置顶' : '置顶'">
                    <i class="iconfont" :class="record.pinned_flag ? 'icon-dingzhu' : 'icon-weizhiding'"></i>
                </button>
                <button class="action-btn" @click.stop="handleCopyOnly" title="仅复制">
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

        <div class="card-content">
            <!-- 文本类型 - 使用智能内容显示 -->
            <template v-if="record.type === 'Text'">
                <SmartContentDisplay 
                    :content="currentTextContent" 
                    :show-type-indicator="false"
                    :max-height="300"
                    :is-truncated="record.content_truncated || false"
                    :on-load-full-content="loadFullContentForDisplay"
                    @copy="handleSmartCopy"
                    @update:content="handleContentUpdate"
                />
            </template>

            <!-- 图片类型 -->
            <template v-else-if="record.type === 'Image'">
                <div class="image-container" ref="imageContainer">
                    <!-- 已加载的图片 -->
                    <img v-if="imageBase64Data && !imageError" :src="imageBase64Data" class="image-preview" 
                        @click.stop="showImagePreview = true" />
                    
                    <!-- 加载中状态 -->
                    <div v-else-if="!isDownloadingFromCloud && (isLoadingImage || (!shouldLoadImage && !imageError))" class="image-placeholder" @click="triggerImageLoad">
                        <div v-if="isLoadingImage" class="placeholder-spinner"></div>
                        <i v-else class="iconfont icon-image placeholder-icon"></i>
                        <span class="loading-text">{{ getImageLoadingText }}</span>
                        <div v-if="record.image_info" class="image-meta">
                            {{ formatFileSize(record.image_info.size) }}
                        </div>
                    </div>
                    
                    <!-- 错误状态 -->
                    <div v-if="imageError && !isDownloadingFromCloud" class="image-error">
                        <i class="iconfont icon-tishi"></i>
                        <span class="error-text">图片加载失败</span>
                        <button class="retry-btn" @click="loadImageBase64">重试</button>
                    </div>
                    
                    <!-- 云端下载状态 -->
                    <div v-if="isDownloadingFromCloud" class="image-downloading">
                        <div class="downloading-spinner"></div>
                        <i class="iconfont icon-cloud downloading-icon"></i>
                        <span class="downloading-text">正在从云端下载...</span>
                        <div v-if="record.image_info" class="image-meta">
                            {{ formatFileSize(record.image_info.size) }}
                        </div>
                    </div>
                </div>
            </template>

            <!-- 文件类型 -->
            <template v-else-if="record.type === 'File'">
                <div class="file-content">
                    <!-- 云端下载状态显示 -->
                    <div v-if="isDownloadingFromCloud" class="file-downloading">
                        <div class="downloading-container">
                            <div class="downloading-spinner"></div>
                            <i class="iconfont icon-cloud downloading-icon"></i>
                            <div class="downloading-info">
                                <span class="downloading-text">正在从云端下载文件...</span>
                                <span class="downloading-desc">下载完成后将自动显示文件信息</span>
                            </div>
                        </div>
                    </div>
                    <div v-else class="file-list">
                        <div v-for="(file, index) in fileList" :key="index" class="file-item">
                            <div class="file-icon-wrapper">
                                <i class="iconfont icon-file"></i>
                            </div>
                            <div class="file-info">
                                <span class="file-name" :title="file.path">{{ getFileName(file.path) }}</span>
                                <span class="file-meta">
                                    {{ formatFileSize(file.size) }} · {{ file.type || getFileType(file.path) }}
                                    <span v-if="file.size === -1" class="file-status file-missing">（文件不存在）</span>
                                    <span v-else-if="file.size === -2" class="file-status file-error">（无法读取）</span>
                                </span>
                            </div>
                            <button v-if="fileList.length > 1" class="single-copy-btn" @click.stop="handleCopySingleFile(file.path)" 
                                    title="仅复制此文件">
                                仅复制此文件
                            </button>
                        </div>
                    </div>
                    <div class="file-count" v-if="!isDownloadingFromCloud && fileList.length > 1">
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
                <SmartContentDisplay 
                    :content="record.content" 
                    :show-type-indicator="false"
                    :max-height="300"
                    @copy="handleSmartCopy"
                />
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
    <vue-easy-lightbox v-if="record.type === 'Image' && imageBase64Data" :visible="showImagePreview"
        :imgs="[{ src: imageBase64Data }]" :index="0" @hide="showImagePreview = false" />

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

    <!-- 自动粘贴警告对话框 -->
    <template v-if="showAutoPasteWarning">
        <div class="confirm-mask" @click.self="cancelAutoPaste">
            <div class="confirm-dialog auto-paste-warning-dialog">
                <div class="warning-header">
                    <div class="warning-icon">⚠️</div>
                    <div class="warning-title">自动粘贴提醒</div>
                </div>
                <div class="warning-content">
                    <p class="warning-description">您即将使用自动粘贴功能，请注意：</p>
                    <ul class="warning-list">
                        <li>某些应用可能自定义了Ctrl+V快捷键</li>
                        <li>在这些应用中使用自动粘贴可能触发意外操作</li>
                        <li>可根据实际使用情况选择是否开启自动粘贴</li>
                    </ul>
                    <p class="warning-question">是否继续使用自动粘贴？</p>
                </div>
                <div class="confirm-actions">
                    <button class="confirm-btn confirm-cancel" @click="cancelAutoPaste">仅复制</button>
                    <button class="confirm-btn confirm-ok" @click="confirmAutoPaste">继续粘贴</button>
                </div>
            </div>
        </div>
    </template>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, shallowRef, watch } from 'vue';
import { formatDistanceToNow } from 'date-fns';
import { zhCN } from 'date-fns/locale';
import { invoke } from '@tauri-apps/api/core';
import VueEasyLightbox from 'vue-easy-lightbox';
import SmartContentDisplay from './SmartContentDisplay.vue';
import { clipApi, settingsApi, isSuccess } from '../utils/api';

// 导入全局类型
import type { ClipRecord } from '../types/global';

// 组件 props 定义

const props = defineProps<{
    record: ClipRecord;
    isMobile: boolean;
    cloudSyncEnabled: boolean;
}>();

const emit = defineEmits<{
    (e: 'click', record: ClipRecord): void;
    (e: 'copy', record: ClipRecord): void;
    (e: 'delete', record: ClipRecord): void;
    (e: 'pin', record: ClipRecord): void;
}>();

const isImageLoaded = ref(false);
const imageError = ref(false);
const showImagePreview = ref(false);
const showTip = ref(false);
const showConfirm = ref(false);
const showAutoPasteWarning = ref(false);

// 大文本处理状态
const hasLoadedFullContent = ref(false);
const fullTextContent = ref('');
const currentTextContent = computed(() => {
    return hasLoadedFullContent.value ? fullTextContent.value : props.record.content;
});

// 图片懒加载相关状态
const imageBase64Data = shallowRef<string>('');
const isLoadingImage = ref(false);
const shouldLoadImage = ref(false);

// showMessageBar 现在通过全局错误处理器自动处理，不需要手动注入

// 懒加载图片base64数据
const loadImageBase64 = async () => {
    if (isLoadingImage.value || imageBase64Data.value || props.record.type !== 'Image') {
        return;
    }
    
    isLoadingImage.value = true;
    try {
        const response = await clipApi.getImageBase64(props.record.id);
        
        if (isSuccess(response)) {
            imageBase64Data.value = response.data.base64_data;
        } else {
            imageError.value = true;
            return;
        }
        
        // 预加载图片确保能正常显示
        const img = new Image();
        img.onload = () => {
            isImageLoaded.value = true;
            imageError.value = false;
        };
        img.onerror = () => {
            isImageLoaded.value = false;
            imageError.value = true;
        };
        img.src = imageBase64Data.value;
        
    } catch (error) {
        console.error('加载图片失败:', error);
        imageError.value = true;
    } finally {
        isLoadingImage.value = false;
    }
};

// 触发图片加载（可见时或用户交互时）
const triggerImageLoad = () => {
    // 如果正在从云端下载，不触发本地加载
    if (isDownloadingFromCloud.value) {
        return;
    }
    if (!shouldLoadImage.value && props.record.type === 'Image') {
        shouldLoadImage.value = true;
        loadImageBase64();
    }
};

// 监听同步状态变化，当从云端下载完成时自动加载图片
watch(() => props.record.sync_flag, (newFlag, oldFlag) => {
    // 当从同步中(1)变为已同步(2)时，且是图片类型，自动加载图片
    if (oldFlag === 1 && newFlag === 2 && props.record.type === 'Image') {
        console.log('云端下载完成，自动加载图片:', props.record.id);
        // 稍微延迟一下再加载，确保状态更新完成
        setTimeout(() => {
            if (!shouldLoadImage.value && !imageBase64Data.value) {
                shouldLoadImage.value = true;
                loadImageBase64();
            }
        }, 100);
    }
});

// 检查是否需要显示自动粘贴提示
const checkFirstAutoPasteUsage = () => {
    const hasShownWarning = localStorage.getItem('auto_paste_warning_shown');
    if (!hasShownWarning) {
        showAutoPasteWarning.value = true;
        return false; // 需要等待用户确认
    }
    return true;
};

// 确认使用自动粘贴
const confirmAutoPaste = async () => {
    localStorage.setItem('auto_paste_warning_shown', 'true');
    showAutoPasteWarning.value = false;
    
    // 继续执行自动粘贴
    const response = await clipApi.copyRecord(props.record.id);
    if (isSuccess(response)) {
        emit('click', props.record);
    }
    // 错误处理由全局错误处理器自动处理
};

// 取消自动粘贴，只复制
const cancelAutoPaste = async () => {
    localStorage.setItem('auto_paste_warning_shown', 'true');
    showAutoPasteWarning.value = false;
    await handleCopyOnly();
};

// 双击卡片触发复制和自动粘贴
const handleCardDoubleClick = async () => {
    // 检查自动粘贴设置
    const settingsResponse = await settingsApi.loadSettings();
    if (isSuccess(settingsResponse)) {
        const settings = settingsResponse.data;
        if (settings.auto_paste === 1) {
            // 如果开启了自动粘贴，检查是否需要显示首次使用提示
            if (!checkFirstAutoPasteUsage()) {
                // 需要显示提示框，等待用户确认
                return;
            }
        }
    }

    const response = await clipApi.copyRecord(props.record.id);
    if (isSuccess(response)) {
        emit('click', props.record);
    }
    // 错误处理由全局错误处理器自动处理
};

// 复制按钮只复制，不触发自动粘贴
const handleCopyOnly = async () => {
    await clipApi.copyRecordNoPaste(props.record.id);
    // 错误处理由全局错误处理器自动处理
};

// 为 SmartContentDisplay 提供的全量加载函数
const loadFullContentForDisplay = async (): Promise<string> => {
    if (hasLoadedFullContent.value) {
        return fullTextContent.value;
    }
    
    const result = await invoke('get_full_text_content', {
        param: { record_id: props.record.id }
    }) as { content: string };
    
    fullTextContent.value = result.content;
    hasLoadedFullContent.value = true;
    
    return result.content;
};

// 处理内容更新事件
const handleContentUpdate = (newContent: string) => {
    fullTextContent.value = newContent;
    hasLoadedFullContent.value = true;
};

// 智能内容复制处理
const handleSmartCopy = (_content: string) => {
    // 移除成功提示，让操作更简洁
};

// 复制单个文件
const handleCopySingleFile = async (filePath: string) => {
    await clipApi.copySingleFile(props.record.id, filePath);
    // 错误处理由全局错误处理器自动处理
};

const formatTime = (timestamp: number) => {
    return formatDistanceToNow(new Date(timestamp), {
        addSuffix: true,
        locale: zhCN
    });
};

const formatFileSize = (bytes: number) => {
    if (bytes === -1) return '未知'; // 文件不存在
    if (bytes === -2) return '未知'; // 无法读取元数据
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
    await clipApi.imageSaveAs(props.record.id);
};

const handleDelete = async () => {
    showConfirm.value = true;
};

const confirmDelete = async () => {
    await clipApi.deleteRecord(props.record.id);
    emit('delete', props.record);
    showConfirm.value = false;
};

const cancelDelete = () => {
    showConfirm.value = false;
};

const handlePin = async () => {
    const newPinnedFlag = !props.record.pinned_flag ? 1 : 0;
    await clipApi.setPinned(props.record.id, newPinnedFlag);
    emit('pin', props.record);
};

const getFileName = (filePath: string) => {
    return filePath.split(/[\\/]/).pop() || filePath;
};

// 检测是否正在从云端下载
const isDownloadingFromCloud = computed(() => {
    // sync_flag为1表示同步中
    // cloud_source为1表示云端数据，为0或undefined表示本地数据
    // 但是新创建的本地截图cloud_source可能暂时为undefined
    // 所以只有明确是云端数据(cloud_source=1)且正在同步才显示下载状态
    return props.record.sync_flag === 1 && props.record.cloud_source === 1;
});

// 获取同步状态的提示文本
const getSyncingTitle = computed(() => {
    if (props.record.sync_flag === 1) {
        // 根据cloud_source判断是本地同步还是云端下载
        if (props.record.cloud_source === 1) {
            if (props.record.type === 'Image') {
                return '正在从云端下载图片...';
            } else if (props.record.type === 'File') {
                return '正在从云端下载文件...';
            }
            return '正在从云端下载...';
        } else {
            return '同步中';
        }
    }
    return '同步中';
});

// 获取图片加载状态文本
const getImageLoadingText = computed(() => {
    if (props.record.sync_flag === 2 && props.record.cloud_source === 1) {
        // 如果是云端数据且已下载完成，但还没加载图片
        return isLoadingImage.value ? '加载中...' : '点击加载图片';
    }
    return isLoadingImage.value ? '加载中...' : '点击加载图片';
});

// 图片容器引用
const imageContainer = ref<HTMLElement>();

// 使用 Intersection Observer 实现可见时自动加载
onMounted(() => {
    if (props.record.type === 'Image' && imageContainer.value) {
        const observer = new IntersectionObserver(
            (entries) => {
                entries.forEach((entry) => {
                    if (entry.isIntersecting && !shouldLoadImage.value && !isDownloadingFromCloud.value) {
                        // 如果是云端数据且已下载完成，或者是本地数据，则加载图片
                        const isCloudCompleted = props.record.sync_flag === 2 && props.record.cloud_source === 1;
                        const isLocal = props.record.cloud_source === 0 || props.record.cloud_source === undefined;
                        
                        if (isCloudCompleted || isLocal) {
                            triggerImageLoad();
                            observer.unobserve(entry.target);
                        }
                    }
                });
            },
            {
                rootMargin: '50px', // 提前50px开始加载
                threshold: 0.1
            }
        );
        
        observer.observe(imageContainer.value);
        
        // 组件卸载时清理observer
        onUnmounted(() => {
            observer.disconnect();
        });
    }
});
</script>

<style scoped>
.clip-card {
    background: var(--card-bg);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-md);
    margin: 0 var(--spacing-xl) var(--card-margin) var(--spacing-xl);
    transition: all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
    cursor: pointer;
    border: var(--border-width) solid var(--border-color);
    position: relative;
    overflow: hidden;
}

.clip-card-hover:hover {
    box-shadow: var(--shadow-lg);
    border-color: var(--border-hover-color);
    transform: translateY(-3px);
}

.clip-card.is-pinned {
    background: var(--pinned-bg, #f8fafc);
    border-left: var(--spacing-xs) solid var(--primary-color);
    box-shadow: 0 4px 12px rgba(44, 122, 123, 0.12);
}

.sync-status {
    font-size: 12px;
    color: var(--text-secondary, #a0aec0);
    font-weight: 500;
    display: flex;
    align-items: center;
    gap: 4px;
}

.sync-unsynced { color: #f39c12; }
.sync-syncing { color: #3498db; }
.sync-synced { color: #2ecc71; }
.sync-skipped { color: #95a5a6; }

.sync-loading {
  display: inline-block;
  margin-right: 4px;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.clip-card.sync-flag-0 { border-color: #f39c12; }
.clip-card.sync-flag-1 { 
  border-color: #3498db; 
  background: #ecf6fb;
  animation: pulse-sync 2s ease-in-out infinite;
}
.clip-card.sync-flag-2 { border-color: #2ecc71; background: #f0fff0; }
.clip-card.sync-flag-3 { border-color: #95a5a6; background: #f8f9fa; }

@keyframes pulse-sync {
  0%, 100% { 
    background: #ecf6fb;
    box-shadow: var(--shadow-md);
  }
  50% { 
    background: #d4edda;
    box-shadow: 0 4px 12px rgba(52, 152, 219, 0.15);
  }
}

.sync-status-indicator {
  position: absolute;
  top: 8px;
  right: 8px;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 10;
  font-size: 12px;
}

.sync-unsynced {
  background: #f39c12;
  color: white;
}

.sync-syncing {
  background: #3498db;
  color: white;
}

.sync-synced {
  background: #2ecc71;
  color: white;
}

.sync-skipped {
  background: #95a5a6;
  color: white;
}

.sync-loading {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: var(--spacing-md) var(--spacing-lg);
    border-bottom: var(--border-width) solid var(--border-color);
    background: #f8fafc;
    position: relative;
}

.card-left {
    display: flex;
    align-items: center;
    gap: var(--spacing-lg);
}

.card-type {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    color: var(--text-secondary);
    font-size: 13px;
    font-weight: 500;
}

.type-text {
    font-weight: 500;
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

/* 平滑滚动条显示 - 支持Firefox */
.text-content.scroll-visible {
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-thumb, #cbd5e1) var(--scrollbar-track, #f1f5f9);
}

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
    /* 允许文本选择 */
    -webkit-user-select: text;
    -moz-user-select: text;
    -ms-user-select: text;
    user-select: text;
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
    cursor: pointer;
    transition: all 0.2s ease;
    background: var(--image-placeholder-bg, #f8fafc);
}

.image-placeholder:hover {
    background: var(--image-placeholder-hover-bg, #e2e8f0);
}

.placeholder-icon {
    font-size: 48px;
    opacity: 0.6;
    color: var(--text-secondary, #64748b);
    transition: all 0.2s ease;
}

.image-placeholder:hover .placeholder-icon {
    opacity: 0.8;
    transform: scale(1.1);
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
    font-weight: 500;
}

.image-meta {
    font-size: 11px;
    color: var(--text-tertiary, #94a3b8);
    margin-top: 4px;
}

.image-size-warning {
    color: #dd6b20;
    font-weight: 500;
    margin-left: 4px;
}

.image-error {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    gap: 12px;
    background: var(--error-bg, #fed7d7);
    color: var(--error-color, #e53e3e);
    border-radius: 8px;
}

.image-error .iconfont {
    font-size: 32px;
    opacity: 0.6;
}

.error-text {
    font-size: 12px;
    color: var(--error-color, #e53e3e);
}

.retry-btn {
    background: var(--primary-color, #2c7a7b);
    color: white;
    border: none;
    border-radius: 4px;
    padding: 6px 12px;
    font-size: 11px;
    cursor: pointer;
    transition: all 0.2s ease;
}

.retry-btn:hover {
    background: var(--primary-hover-color, #234e52);
    transform: translateY(-1px);
}

/* 云端下载状态样式 - 图片 */
.image-downloading {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    gap: 12px;
    background: linear-gradient(135deg, #e3f2fd 0%, #f0f9ff 100%);
    border-radius: 8px;
    position: relative;
}

.downloading-spinner {
    width: 32px;
    height: 32px;
    border: 3px solid rgba(33, 150, 243, 0.2);
    border-top-color: #2196f3;
    border-radius: 50%;
    animation: download-spin 1.2s linear infinite;
}

@keyframes download-spin {
    0% { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
}

.downloading-icon {
    font-size: 24px;
    color: #2196f3;
    opacity: 0.8;
    animation: download-pulse 2s ease-in-out infinite;
}

@keyframes download-pulse {
    0%, 100% { 
        opacity: 0.8; 
        transform: scale(1);
    }
    50% { 
        opacity: 1; 
        transform: scale(1.1);
    }
}

.downloading-text {
    font-size: 13px;
    color: #1976d2;
    font-weight: 500;
    text-align: center;
}

/* 云端下载状态样式 - 文件 */
.file-downloading {
    padding: 20px;
    background: linear-gradient(135deg, #e8f5e8 0%, #f0fff4 100%);
    border-radius: 8px;
    border: 1px solid #4caf50;
}

.downloading-container {
    display: flex;
    align-items: center;
    gap: 16px;
}

.file-downloading .downloading-spinner {
    width: 24px;
    height: 24px;
    border: 2px solid rgba(76, 175, 80, 0.3);
    border-top-color: #4caf50;
    flex-shrink: 0;
}

.file-downloading .downloading-icon {
    font-size: 20px;
    color: #4caf50;
    flex-shrink: 0;
}

.downloading-info {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1;
}

.file-downloading .downloading-text {
    font-size: 14px;
    color: #2e7d32;
    font-weight: 500;
    margin: 0;
}

.downloading-desc {
    font-size: 12px;
    color: #4caf50;
    opacity: 0.8;
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
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-thumb, #cbd5e1) var(--scrollbar-track, #f1f5f9);
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

.file-status {
    font-size: 10px;
    font-weight: 500;
    margin-left: 4px;
    padding: 1px 4px;
    border-radius: 3px;
}

.file-missing {
    color: #e53e3e;
    background: rgba(229, 62, 62, 0.1);
}

.file-error {
    color: #d69e2e;
    background: rgba(214, 158, 46, 0.1);
}

.file-oversized {
    color: #dd6b20;
    background: rgba(221, 107, 32, 0.1);
}

.file-count {
    margin-top: 10px;
    font-size: 11px;
    color: var(--text-secondary, #64748b);
    text-align: right;
    padding-right: 6px;
}

.oversized-warning {
    margin-top: 8px;
    padding: 6px 8px;
    background: rgba(221, 107, 32, 0.08);
    border: 1px solid rgba(221, 107, 32, 0.2);
    border-radius: 4px;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: #dd6b20;
}

.oversized-warning .iconfont {
    font-size: 12px;
    flex-shrink: 0;
}

/* 单文件复制按钮样式 */
.single-copy-btn {
    padding: 4px 8px;
    background: var(--primary-color, #2c7a7b);
    color: white;
    border: none;
    border-radius: 4px;
    font-size: 11px;
    cursor: pointer;
    transition: all 0.2s ease;
    flex-shrink: 0;
    opacity: 0.8;
}

.single-copy-btn:hover {
    background: var(--primary-hover, #285e61);
    opacity: 1;
    transform: translateY(-1px);
}

.single-copy-btn:active {
    transform: translateY(0);
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
    /* 允许文本选择 */
    -webkit-user-select: text;
    -moz-user-select: text;
    -ms-user-select: text;
    user-select: text;
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

/* 自动粘贴警告对话框样式 */
.auto-paste-warning-dialog {
    min-width: 360px;
    max-width: 420px;
    padding: 24px 28px;
    text-align: left;
}

.warning-header {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 16px;
    padding-bottom: 12px;
    border-bottom: 1px solid #ffeaa7;
}

.warning-icon {
    font-size: 24px;
    flex-shrink: 0;
}

.warning-title {
    font-size: 18px;
    font-weight: 600;
    color: #d63031;
    margin: 0;
}

.warning-content {
    margin-bottom: 20px;
}

.warning-description {
    font-size: 15px;
    color: #2d3436;
    margin: 0 0 12px 0;
    font-weight: 500;
}

.warning-list {
    margin: 12px 0 16px 20px;
    padding: 0;
    list-style: none;
}

.warning-list li {
    position: relative;
    font-size: 14px;
    color: #636e72;
    line-height: 1.5;
    margin-bottom: 6px;
    padding-left: 16px;
}

.warning-list li::before {
    content: '•';
    position: absolute;
    left: 0;
    color: #fdcb6e;
    font-weight: bold;
    font-size: 16px;
}

.warning-question {
    font-size: 15px;
    color: #2d3436;
    margin: 16px 0 0 0;
    font-weight: 500;
}

.auto-paste-warning-dialog .confirm-actions {
    margin-top: 20px;
    justify-content: flex-end;
    gap: 12px;
}

.auto-paste-warning-dialog .confirm-btn {
    min-width: 80px;
    padding: 8px 16px;
    font-size: 14px;
}

.auto-paste-warning-dialog .confirm-cancel {
    background: #f8f9fa;
    color: #636e72;
    border: 1px solid #ddd;
}

.auto-paste-warning-dialog .confirm-cancel:hover {
    background: #e9ecef;
    border-color: #adb5bd;
}

.auto-paste-warning-dialog .confirm-ok {
    background: #00b894;
    color: white;
    border: none;
}

.auto-paste-warning-dialog .confirm-ok:hover {
    background: #00a085;
}

/* 响应式设计 - 针对不同窗口尺寸优化 */
@media (max-width: 480px) {
  .clip-card {
    margin: 0 12px 12px 12px;
    border-radius: 8px;
  }
  
  .card-header {
    padding: 10px 12px;
  }
  
  .card-left {
    gap: 10px;
  }
  
  .card-type .type-text {
    font-size: 12px;
  }
  
  .time-text {
    font-size: 11px;
  }
  
  .card-actions {
    gap: 6px;
  }
  
  .action-btn {
    width: 20px;
    height: 20px;
    font-size: 12px;
  }
  
  .card-content {
    padding: 10px 12px 12px 12px;
  }
  
  .image-container {
    height: 160px;
  }
  
  .file-item {
    padding: 8px 10px;
    gap: 8px;
  }
  
  .file-icon-wrapper {
    width: 28px;
    height: 28px;
  }
  
  .file-name {
    font-size: 12px;
  }
  
  .file-meta {
    font-size: 10px;
  }
  
  .single-copy-btn {
    padding: 3px 6px;
    font-size: 10px;
  }
  
  .json-content,
  .code-content,
  .default-content {
    padding: 10px;
    gap: 8px;
  }
  
  .content-icon {
    width: 24px;
    height: 24px;
  }
  
  .json-preview,
  .code-preview {
    font-size: 12px;
    max-height: 120px;
  }
  
  .tip-pop {
    max-width: 180px;
    font-size: 11px;
    padding: 6px 12px;
  }
}

@media (max-width: 360px) {
  .clip-card {
    margin: 0 8px 10px 8px;
  }
  
  .card-header {
    padding: 8px 10px;
  }
  
  .card-left {
    gap: 8px;
  }
  
  .card-actions {
    gap: 4px;
  }
  
  .action-btn {
    width: 18px;
    height: 18px;
    font-size: 11px;
  }
  
  .card-content {
    padding: 8px 10px 10px 10px;
  }
  
  .image-container {
    height: 140px;
  }
  
  .file-list {
    max-height: 120px;
  }
  
  .file-item {
    padding: 6px 8px;
    gap: 6px;
  }
  
  .json-content,
  .code-content,
  .default-content {
    padding: 8px;
    gap: 6px;
  }
  
  .json-preview,
  .code-preview {
    font-size: 12px;
    max-height: 100px;
  }
}

/* 针对Tauri窗口的特殊优化 - 窄窗口模式 */
@media (max-width: 500px) and (min-height: 600px) {
  .clip-card {
    margin: 0 16px 14px 16px;
  }
  
  .card-header {
    padding: 10px 14px;
  }
  
  .card-left {
    gap: 12px;
  }
  
  .card-type .type-text {
    font-size: 12px;
  }
  
  .time-text {
    font-size: 11px;
  }
}

/* Windows和macOS平台差异化处理 */
@supports (-webkit-backdrop-filter: blur()) {
  /* macOS样式优化 */
  .clip-card {
    backdrop-filter: blur(10px);
    background: rgba(255, 255, 255, 0.9);
  }
  
  .clip-card-hover:hover {
    backdrop-filter: blur(15px);
    background: rgba(255, 255, 255, 0.95);
  }
}

/* 暗色模式下的响应式优化 */
@media (prefers-color-scheme: dark) {
  @media (max-width: 480px) {
    .clip-card {
      --card-bg: #2d2d2d;
      --border-color: #3d3d3d;
    }
    
    .file-item {
      --item-bg: #3a3a3a;
      --item-hover-bg: #404040;
      --border-color: #4a4a4a;
    }
    
    .json-content,
    .code-content,
    .default-content {
      --item-bg: #3a3a3a;
      --border-color: #4a4a4a;
    }
  }
  
  @supports (-webkit-backdrop-filter: blur()) {
    .clip-card {
      backdrop-filter: blur(10px);
      background: rgba(45, 45, 45, 0.9);
    }
    
    .clip-card-hover:hover {
      backdrop-filter: blur(15px);
      background: rgba(45, 45, 45, 0.95);
    }
  }
}

/* 高DPI屏幕优化 */
@media (-webkit-min-device-pixel-ratio: 2), (min-resolution: 192dpi) {
  .action-btn,
  .single-copy-btn {
    border: 0.5px solid transparent;
  }
  
  .clip-card {
    border-width: 0.5px;
  }
}

.sync-dot {
  display: inline-flex;
  align-items: center;
  margin-left: 6px;
}

.sync-unsynced {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #f39c12;
}

.sync-syncing {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #3498db;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 8px;
  color: white;
  animation: pulse-scale 1s ease-in-out infinite;
}

@keyframes pulse-scale {
  0%, 100% { 
    transform: scale(1);
  }
  50% { 
    transform: scale(1.3);
  }
}

.sync-synced {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #2ecc71;
}

.sync-skipped {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #95a5a6;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 6px;
  color: white;
  position: relative;
}

.sync-loading {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

</style>