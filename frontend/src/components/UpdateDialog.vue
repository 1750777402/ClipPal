<template>
  <div v-if="isVisible" class="update-dialog-overlay" @click.self="handleClose">
    <div class="update-dialog">
      <!-- 标题 -->
      <div class="dialog-header">
        <h2>{{ updateState === 'checking' ? '检查更新中...' : '发现新版本' }}</h2>
        <button v-if="updateState !== 'downloading'" class="close-btn" @click="handleClose">×</button>
      </div>

      <!-- 内容区域 -->
      <div class="dialog-content">
        <!-- 检查中状态 -->
        <div v-if="updateState === 'checking'" class="checking-state">
          <div class="spinner"></div>
          <p>正在检查更新...</p>
        </div>

        <!-- 无更新状态 -->
        <div v-else-if="updateState === 'no-update'" class="no-update-state">
          <div class="icon">✓</div>
          <p>已是最新版本</p>
          <p class="version-info">当前版本: {{ currentVersion }}</p>
          <button class="btn-close" @click="handleClose">关闭</button>
        </div>

        <!-- 有更新状态 -->
        <div v-else-if="updateState === 'has-update'" class="has-update-state">
          <div class="version-info">
            <p><strong>当前版本:</strong> {{ updateInfo?.current_version }}</p>
            <p><strong>最新版本:</strong> {{ updateInfo?.latest_version }}</p>
          </div>

          <!-- 更新日志 -->
          <div v-if="updateInfo?.body" class="changelog">
            <h3>更新内容:</h3>
            <div class="changelog-content" v-html="formatChangelog(updateInfo.body)"></div>
          </div>

          <div class="action-buttons">
            <button class="btn-cancel" @click="handleClose">稍后更新</button>
            <button class="btn-update" @click="handleUpdate">立即更新</button>
          </div>
        </div>

        <!-- 下载中状态 -->
        <div v-else-if="updateState === 'downloading'" class="downloading-state">
          <p>正在下载更新...</p>
          <div class="progress-bar">
            <div class="progress-fill" :style="{ width: downloadProgress + '%' }"></div>
          </div>
          <p class="progress-text">{{ downloadProgress }}%</p>
        </div>

        <!-- 安装中状态 -->
        <div v-else-if="updateState === 'installing'" class="installing-state">
          <div class="spinner"></div>
          <p>正在安装更新...</p>
          <p class="hint">安装完成后应用将自动重启</p>
        </div>

        <!-- 成功状态 -->
        <div v-else-if="updateState === 'success'" class="success-state">
          <div class="icon">✓</div>
          <p>更新成功!</p>
          <p class="hint">应用将在几秒后重启</p>
        </div>

        <!-- 错误状态 -->
        <div v-else-if="updateState === 'error'" class="error-state">
          <div class="icon">✕</div>
          <p>更新失败</p>
          <p class="error-message">{{ errorMessage }}</p>
          <button class="btn-retry" @click="handleRetry">重试</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { UpdateInfo } from '../types/global'

interface Props {
  modelValue: boolean
}

interface Emits {
  (e: 'update:modelValue', value: boolean): void
}

const props = defineProps<Props>()
const emit = defineEmits<Emits>()

const isVisible = computed({
  get: () => props.modelValue,
  set: (value) => emit('update:modelValue', value)
})

type UpdateState = 'checking' | 'no-update' | 'has-update' | 'downloading' | 'installing' | 'success' | 'error'

const updateState = ref<UpdateState>('checking')
const updateInfo = ref<UpdateInfo | null>(null)
const downloadProgress = ref(0)
const errorMessage = ref('')
const currentVersion = ref('1.0.6')
const isFromAutoCheck = ref(false) // 标记是否来自自动检查

const formatChangelog = (body: string): string => {
  // 简单的 Markdown 转 HTML
  return body
    .split('\n')
    .map(line => {
      if (line.startsWith('## ')) return `<h3>${line.substring(3)}</h3>`
      if (line.startsWith('- ')) return `<li>${line.substring(2)}</li>`
      if (line.trim()) return `<p>${line}</p>`
      return ''
    })
    .join('')
}

const handleClose = () => {
  if (updateState.value !== 'downloading' && updateState.value !== 'installing') {
    isVisible.value = false
  }
}

const handleRetry = () => {
  checkUpdate()
}

const checkUpdate = async () => {
  updateState.value = 'checking'
  try {
    const result = await invoke<UpdateInfo>('check_soft_version')
    updateInfo.value = result

    if (result.has_update) {
      updateState.value = 'has-update'
    } else {
      updateState.value = 'no-update'
      // 不自动关闭，让用户看到"已是最新版本"的提示
    }
  } catch (error) {
    errorMessage.value = String(error)
    updateState.value = 'error'
  }
}

// 直接设置更新信息（来自后端自动检查）
const setUpdateInfo = (info: UpdateInfo) => {
  updateInfo.value = info
  isFromAutoCheck.value = true

  if (info.has_update) {
    updateState.value = 'has-update'
  } else {
    updateState.value = 'no-update'
  }
}

const handleUpdate = async () => {
  updateState.value = 'downloading'
  downloadProgress.value = 0
  
  try {
    // 模拟下载进度（实际进度由后端通过事件发送）
    const progressInterval = setInterval(() => {
      if (downloadProgress.value < 90) {
        downloadProgress.value += Math.random() * 30
      }
    }, 500)

    const result = await invoke<boolean>('download_and_install_update')
    
    clearInterval(progressInterval)
    downloadProgress.value = 100
    
    if (result) {
      updateState.value = 'installing'
      setTimeout(() => {
        updateState.value = 'success'
      }, 1000)
    }
  } catch (error) {
    errorMessage.value = String(error)
    updateState.value = 'error'
  }
}

// 组件挂载时检查更新
const init = () => {
  if (isVisible.value) {
    checkUpdate()
  }
}

defineExpose({
  init,
  checkUpdate,
  setUpdateInfo
})
</script>

<style scoped>
.update-dialog-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.update-dialog {
  background: white;
  border-radius: 12px;
  box-shadow: 0 10px 40px rgba(0, 0, 0, 0.2);
  width: 90%;
  max-width: 500px;
  overflow: hidden;
}

.dialog-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 20px;
  border-bottom: 1px solid #eee;
}

.dialog-header h2 {
  margin: 0;
  font-size: 18px;
  color: #333;
}

.close-btn {
  background: none;
  border: none;
  font-size: 28px;
  cursor: pointer;
  color: #999;
  padding: 0;
  width: 30px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.close-btn:hover {
  color: #333;
}

.dialog-content {
  padding: 30px 20px;
  min-height: 200px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}

.spinner {
  width: 40px;
  height: 40px;
  border: 4px solid #f3f3f3;
  border-top: 4px solid #3498db;
  border-radius: 50%;
  animation: spin 1s linear infinite;
  margin-bottom: 15px;
}

@keyframes spin {
  0% { transform: rotate(0deg); }
  100% { transform: rotate(360deg); }
}

.checking-state,
.installing-state {
  text-align: center;
}

.checking-state p,
.installing-state p {
  margin: 10px 0;
  color: #666;
}

.hint {
  font-size: 12px;
  color: #999;
  margin-top: 10px;
}

.no-update-state,
.success-state,
.error-state {
  text-align: center;
}

.icon {
  font-size: 48px;
  margin-bottom: 15px;
  color: #27ae60;
}

.error-state .icon {
  color: #e74c3c;
}

.version-info {
  background: #f5f5f5;
  padding: 15px;
  border-radius: 8px;
  margin-bottom: 15px;
  text-align: left;
  font-size: 14px;
}

.version-info p {
  margin: 8px 0;
  color: #666;
}

.changelog {
  background: #f9f9f9;
  border-left: 3px solid #3498db;
  padding: 15px;
  margin: 15px 0;
  border-radius: 4px;
  max-height: 200px;
  overflow-y: auto;
  text-align: left;
}

.changelog h3 {
  margin: 0 0 10px 0;
  font-size: 14px;
  color: #333;
}

.changelog-content {
  font-size: 13px;
  color: #666;
  line-height: 1.6;
}

.changelog-content h3 {
  margin: 10px 0 5px 0;
  font-size: 13px;
}

.changelog-content li {
  margin-left: 20px;
  list-style: disc;
}

.progress-bar {
  width: 100%;
  height: 8px;
  background: #eee;
  border-radius: 4px;
  overflow: hidden;
  margin: 15px 0;
}

.progress-fill {
  height: 100%;
  background: linear-gradient(90deg, #3498db, #2ecc71);
  transition: width 0.3s ease;
}

.progress-text {
  font-size: 14px;
  color: #666;
  margin: 10px 0;
}

.action-buttons {
  display: flex;
  gap: 10px;
  margin-top: 20px;
  width: 100%;
}

.btn-cancel,
.btn-update,
.btn-retry,
.btn-close {
  flex: 1;
  padding: 10px 20px;
  border: none;
  border-radius: 6px;
  font-size: 14px;
  cursor: pointer;
  transition: all 0.3s ease;
}

.btn-cancel {
  background: #ecf0f1;
  color: #333;
}

.btn-cancel:hover {
  background: #d5dbdb;
}

.btn-update,
.btn-retry {
  background: #3498db;
  color: white;
}

.btn-update:hover,
.btn-retry:hover {
  background: #2980b9;
}

.btn-close {
  background: #3498db;
  color: white;
  margin-top: 15px;
}

.btn-close:hover {
  background: #2980b9;
}

.error-message {
  color: #e74c3c;
  font-size: 12px;
  margin: 10px 0;
}
</style>

