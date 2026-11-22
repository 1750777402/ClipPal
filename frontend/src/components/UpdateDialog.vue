<template>
  <div v-if="isVisible" class="update-dialog-overlay" @click.self="handleClose">
    <div class="update-dialog">
      <!-- 标题 -->
      <div class="dialog-header">
        <h2>{{ dialogTitle }}</h2>
        <button v-if="!isBusy" class="close-btn" @click="handleClose">×</button>
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

        <div v-else-if="updateState === 'manual' || updateState === 'unavailable'" class="no-update-state">
          <p>{{ updateState === 'manual' ? '此安装包采用手动更新。请从原来源获取新版安装包。' : '更新源暂未提供适用于当前系统和架构的安装包。' }}</p>
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
          <div v-if="downloadProgress !== null" class="progress-bar" role="progressbar" aria-label="下载进度"
            :aria-valuenow="downloadProgress">
            <div class="progress-fill" :style="{ width: downloadProgress + '%' }"></div>
          </div>
          <div v-else class="spinner" role="progressbar" aria-label="下载中"></div>
          <p class="progress-text">{{ downloadProgress === null ? `已下载 ${downloadedText}` : `${downloadProgress}%` }}</p>
        </div>

        <div v-else-if="updateState === 'verifying'" class="installing-state">
          <div class="spinner"></div>
          <p>正在校验更新包签名...</p>
        </div>

        <!-- 安装中状态 -->
        <div v-else-if="updateState === 'installing'" class="installing-state">
          <div class="spinner"></div>
          <p>正在安装更新...</p>
          <p class="hint">安装期间请保持应用运行</p>
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
import { computed, ref } from 'vue'
import { Channel } from '@tauri-apps/api/core'
import DOMPurify from 'dompurify'
import { marked } from 'marked'
import { apiInvoke, isSuccess } from '../utils/api'
import type { UpdateInfo, UpdateProgress } from '../types/global'

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

type UpdateState = 'checking' | 'no-update' | 'manual' | 'unavailable' | 'has-update'
  | 'downloading' | 'verifying' | 'installing' | 'success' | 'error'

const updateState = ref<UpdateState>('checking')
const updateInfo = ref<UpdateInfo | null>(null)
const downloaded = ref(0)
const downloadTotal = ref<number | null>(null)
const errorMessage = ref('')
let checkInFlight: Promise<void> | null = null
let manuallyOpened = false

const isBusy = computed(() =>
  ['downloading', 'verifying', 'installing', 'success'].includes(updateState.value)
)

const downloadProgress = computed(() => downloadTotal.value
  ? Math.min(100, Math.floor(downloaded.value / downloadTotal.value * 100)) : null)
const downloadedText = computed(() => downloaded.value < 1024 * 1024
  ? `${(downloaded.value / 1024).toFixed(1)} KB` : `${(downloaded.value / (1024 * 1024)).toFixed(1)} MB`)

const currentVersion = computed(() => updateInfo.value?.current_version ?? '')

const dialogTitle = computed(() => {
  switch (updateState.value) {
    case 'checking':
      return '检查更新中...'
    case 'no-update':
    case 'manual':
    case 'unavailable':
      return '软件更新'
    case 'has-update':
      return '发现新版本'
    case 'downloading':
      return '下载更新'
    case 'verifying':
      return '校验更新'
    case 'installing':
      return '安装更新'
    case 'success':
      return '更新完成'
    case 'error':
      return '更新失败'
  }
})

const formatChangelog = (body: string): string => {
  const rendered = marked.parse(body, { async: false })
  return DOMPurify.sanitize(rendered)
}

const handleClose = () => {
  if (!isBusy.value) {
    isVisible.value = false
  }
}

const handleRetry = () => {
  void checkUpdate()
}

/** 手动检查接管已有的启动检查，避免同一时间发出两次请求。 */
const checkUpdate = (manual = true) => {
  if (isBusy.value) return
  if (!checkInFlight) manuallyOpened = false
  if (manual) {
    manuallyOpened = true
    isVisible.value = true
  }
  if (checkInFlight) return checkInFlight

  updateState.value = 'checking'
  updateInfo.value = null
  errorMessage.value = ''
  checkInFlight = (async () => {
    const response = await apiInvoke<UpdateInfo>('check_soft_version')
    if (!isSuccess(response)) {
      errorMessage.value = response.error || '检查更新失败'
      updateState.value = 'error'
      return
    }
    const result = response.data
    updateInfo.value = result
    const states = { available: 'has-update', up_to_date: 'no-update', manual: 'manual', unavailable: 'unavailable' } as const
    updateState.value = states[result.status]
    // 手动打开后如果用户已关闭窗口，迟到的结果也不重新弹出。
    if (!manuallyOpened && result.status === 'available') isVisible.value = true
  })().finally(() => { checkInFlight = null })
  return checkInFlight
}

/** 提交当前弹窗展示的版本，只消费本次请求的进度。 */
const handleUpdate = async () => {
  const version = updateInfo.value?.latest_version
  if (updateState.value !== 'has-update' || !version) return
  updateState.value = 'downloading'
  downloaded.value = 0
  downloadTotal.value = null
  errorMessage.value = ''
  const channel = new Channel<UpdateProgress>((progress) => {
    if (!isBusy.value || updateState.value === 'success') return
    if (progress.phase === 'downloading') {
      downloaded.value = progress.downloaded
      downloadTotal.value = progress.total
    } else {
      updateState.value = progress.phase
    }
  })
  const response = await apiInvoke<null>('download_and_install_update', { version, onProgress: channel })
  if (response.success) {
    updateState.value = 'success'
  } else {
    errorMessage.value = response.error || '下载安装更新失败'
    updateState.value = 'error'
  }
}

defineExpose({ checkUpdate })
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

