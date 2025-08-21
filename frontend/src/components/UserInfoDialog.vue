<template>
  <div v-if="visible" class="user-info-overlay" @click="handleOverlayClick">
    <div class="user-info-container" @click.stop>
      <div class="dialog-header">
        <h2 class="dialog-title">个人信息</h2>
        <button class="close-btn" @click="close" type="button">×</button>
      </div>
      
      <div class="dialog-content">
        <div class="user-avatar-section">
          <div class="large-user-avatar">
            {{ userInfo?.account?.charAt(0)?.toUpperCase() || 'U' }}
          </div>
          <p class="user-account">{{ userInfo?.account || '未知用户' }}</p>
        </div>
        
        <div class="user-details">
          <div class="detail-item">
            <label>账号</label>
            <span>{{ userInfo?.account || '未设置' }}</span>
          </div>
          
          <div class="detail-item">
            <label>昵称</label>
            <span>{{ userInfo?.nickname || '未设置' }}</span>
          </div>
          
          <div class="detail-item">
            <label>邮箱</label>
            <span>{{ userInfo?.email || '未设置' }}</span>
          </div>
          
          <div class="detail-item">
            <label>创建时间</label>
            <span>{{ formatDate(userInfo?.created_at) || '未知' }}</span>
          </div>
          
          <div class="detail-item">
            <label>最后登录</label>
            <span>{{ formatDate(userInfo?.last_login_time) || '未知' }}</span>
          </div>
        </div>
        
        <div class="dialog-actions">
          <button class="action-btn secondary" @click="close" type="button">
            关闭
          </button>
          <button class="action-btn primary" @click="handleEdit" type="button">
            编辑信息
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { UserInfo } from '../utils/userStore'

interface Props {
  visible: boolean
  userInfo: UserInfo | null
}

interface Emits {
  (e: 'update:visible', value: boolean): void
  (e: 'edit'): void
}

const props = defineProps<Props>()
const emit = defineEmits<Emits>()

const close = () => {
  emit('update:visible', false)
}

const handleOverlayClick = () => {
  close()
}

const handleEdit = () => {
  emit('edit')
  close()
}

// 格式化日期
const formatDate = (dateString: string | undefined): string => {
  if (!dateString) return '未知'
  
  try {
    const date = new Date(dateString)
    return date.toLocaleString('zh-CN', {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit'
    })
  } catch (error) {
    return '格式错误'
  }
}
</script>

<style scoped>
.user-info-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  animation: fadeIn 0.3s ease;
}

.user-info-container {
  background: var(--card-bg, #ffffff);
  border-radius: var(--radius-lg, 12px);
  box-shadow: 0 20px 40px rgba(0, 0, 0, 0.15);
  width: 100%;
  max-width: 480px;
  margin: 20px;
  animation: slideUp 0.3s ease;
}

.dialog-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 24px 24px 0 24px;
}

.dialog-title {
  margin: 0;
  font-size: 24px;
  font-weight: 600;
  color: var(--text-primary, #333);
}

.close-btn {
  background: none;
  border: none;
  font-size: 24px;
  color: var(--text-secondary, #666);
  cursor: pointer;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  transition: all 0.2s ease;
}

.close-btn:hover {
  background: var(--bg-hover, #f5f5f5);
  color: var(--text-primary, #333);
}

.dialog-content {
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.user-avatar-section {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
}

.large-user-avatar {
  width: 80px;
  height: 80px;
  border-radius: 50%;
  background: linear-gradient(135deg, var(--header-bg, #2c7a7b), #319795);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 32px;
  font-weight: bold;
  color: white;
  box-shadow: 0 4px 12px rgba(44, 122, 123, 0.3);
}

.user-account {
  font-size: 18px;
  font-weight: 600;
  color: var(--text-primary, #333);
  margin: 0;
}

.user-details {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.detail-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  background: var(--bg-secondary, #f8fafc);
  border-radius: var(--radius-md, 8px);
  border: 1px solid var(--border-light, #e2e8f0);
}

.detail-item label {
  font-weight: 500;
  color: var(--text-secondary, #666);
  min-width: 80px;
}

.detail-item span {
  color: var(--text-primary, #333);
  text-align: right;
  flex: 1;
}

.dialog-actions {
  display: flex;
  gap: 12px;
  justify-content: flex-end;
  margin-top: 8px;
}

.action-btn {
  padding: 10px 20px;
  border: none;
  border-radius: var(--radius-md, 8px);
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
  min-width: 80px;
}

.action-btn.secondary {
  background: var(--bg-secondary, #f8fafc);
  color: var(--text-secondary, #666);
  border: 1px solid var(--border-color, #e2e8f0);
}

.action-btn.secondary:hover {
  background: var(--bg-hover, #e2e8f0);
  color: var(--text-primary, #333);
}

.action-btn.primary {
  background: linear-gradient(135deg, var(--header-bg, #2c7a7b), #319795);
  color: white;
}

.action-btn.primary:hover {
  background: linear-gradient(135deg, #319795, #2dd4bf);
  box-shadow: 0 4px 12px rgba(44, 122, 123, 0.25);
  transform: translateY(-1px);
}

@keyframes fadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

@keyframes slideUp {
  from {
    opacity: 0;
    transform: translateY(20px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

/* 响应式适配 */
@media (max-width: 480px) {
  .user-info-container {
    margin: 10px;
    max-width: none;
  }
  
  .dialog-header, .dialog-content {
    padding-left: 20px;
    padding-right: 20px;
  }
  
  .dialog-title {
    font-size: 20px;
  }
  
  .large-user-avatar {
    width: 64px;
    height: 64px;
    font-size: 24px;
  }
  
  .detail-item {
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
  }
  
  .detail-item span {
    text-align: left;
  }
  
  .dialog-actions {
    flex-direction: column;
  }
  
  .action-btn {
    width: 100%;
  }
}

/* 暗色模式支持 */
@media (prefers-color-scheme: dark) {
  .user-info-container {
    --card-bg: #2d2d2d;
    --text-primary: #e6e6e6;
    --text-secondary: #999999;
    --bg-secondary: #3a3a3a;
    --bg-hover: #3d3d3d;
    --border-color: #3d3d3d;
    --border-light: #404040;
  }
}
</style>