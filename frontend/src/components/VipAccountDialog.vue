<template>
  <div v-if="visible" class="vip-account-overlay" @click="handleOverlayClick">
    <div class="vip-account-container" @click.stop>
      <div class="dialog-header">
        <h2 class="dialog-title">VIP账户</h2>
        <button class="close-btn" @click="close" type="button">×</button>
      </div>
      
      <div class="dialog-content">
        <!-- VIP状态和权益信息区域 -->
        <div class="vip-section" v-if="userStore.isLoggedIn()">
          <div class="section-header">
            <h3 class="section-title">
              <span class="section-icon">👑</span>
              会员状态与权益
              <button 
                v-if="!isRefreshing" 
                class="refresh-btn" 
                @click="refreshVipStatus" 
                title="刷新VIP状态"
                type="button"
              >
                🔄
              </button>
              <span v-else class="refreshing-indicator">🔄</span>
            </h3>
          </div>
          
          <!-- 合并的状态和权益卡片 -->
          <div class="vip-status-card" :class="vipStatusClass">
            <div class="card-header">
              <div class="vip-main-info">
                <div class="vip-icon-large">
                  {{ vipStore.isVip ? '👑' : '🆓' }}
                </div>
                <div class="vip-details">
                  <div class="vip-type">{{ vipStore.vipTypeDisplay }}</div>
                  
                  <div v-if="vipStore.isVip && vipStore.expireTimeDisplay" class="vip-expire-time">
                    到期时间: {{ vipStore.expireTimeDisplay }}
                  </div>
                  <div v-if="remainingDaysText" class="vip-remaining" :class="{
                    'text-warning': vipStore.isExpiringSoon,
                    'text-danger': vipStore.isExpired
                  }">
                    {{ remainingDaysText }}
                  </div>
                </div>
              </div>
              
              <button 
                class="upgrade-btn"
                @click="showVipDialog = true"
                type="button"
              >
                {{ vipStore.isVip ? '管理会员' : '升级VIP' }}
              </button>
            </div>
            
            <!-- 权益信息在同一张卡片内 -->
            <div class="card-benefits">
              <div class="benefits-grid">
                <div class="benefit-item">
                  <span class="benefit-icon">📄</span>
                  <div class="benefit-text">
                    <span class="benefit-label">本地记录</span>
                    <span class="benefit-value">{{ vipStore.maxRecordsLimit }}</span>
                  </div>
                </div>
                <div class="benefit-item">
                  <span class="benefit-icon">☁️</span>
                  <div class="benefit-text">
                    <span class="benefit-label">云同步</span>
                    <span class="benefit-value" :class="{ 'text-primary': vipStore.canCloudSync }">
                      {{ vipStore.canCloudSync ? (vipStore.isVip ? `${vipStore.vipInfo?.max_sync_records || '无限'}条` : '10条体验') : '不支持' }}
                    </span>
                  </div>
                </div>
                <div class="benefit-item">
                  <span class="benefit-icon">📁</span>
                  <div class="benefit-text">
                    <span class="benefit-label">文件上传</span>
                    <span class="benefit-value" :class="{ 'text-primary': vipStore.isVip }">
                      {{ vipStore.isVip ? `${((vipStore.limits?.maxFileSize || 0) / 1024 / 1024).toFixed(0)}MB以下` : '不支持' }}
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
        
        <!-- 未登录状态 -->
        <div class="login-prompt" v-else>
          <div class="prompt-icon">👤</div>
          <div class="prompt-text">请先登录以查看VIP账户信息</div>
          <button class="login-btn" @click="$emit('login')" type="button">
            立即登录
          </button>
        </div>
      </div>
    </div>
  </div>
  
  <!-- VIP升级对话框 -->
  <VipUpgradeDialog v-model="showVipDialog" />
</template>

<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { useVipStore } from '../utils/vipStore'
import { useUserStore } from '../utils/userStore'
import VipUpgradeDialog from './VipUpgradeDialog.vue'

interface Props {
  visible: boolean
}

interface Emits {
  (e: 'update:visible', value: boolean): void
  (e: 'login'): void
}

const props = defineProps<Props>()
const emit = defineEmits<Emits>()

// VIP 状态管理
const vipStore = useVipStore()
const userStore = useUserStore()
const showVipDialog = ref(false)
const isRefreshing = ref(false)

// VIP状态显示逻辑：区分从未开过VIP、VIP过期、VIP有效三种情况
// const vipStatus = computed(() => {
//   const vipInfo = vipStore.vipInfo
//   // 情况1：从未开过VIP（API返回None或无expire_time）
//   if (!vipInfo || !vipInfo.expire_time) {
//     return '普通用户'
//   }
  
//   // 情况2和3：有expire_time，判断是否过期
//   const now = Date.now() / 1000
//   const isExpired = now > vipInfo.expire_time
  
//   if (isExpired) {
//     return 'VIP已过期'  // VIP已过期，但用户身份仍显示为普通用户
//   } else {
//     return 'VIP会员'     // VIP有效
//   }
// })

// 实时刷新VIP状态的功能
const refreshVipStatus = async () => {
  if (isRefreshing.value) return
  
  isRefreshing.value = true
  try {
    const success = await vipStore.refreshStatus()
    if (success) {
      console.log('VIP状态刷新成功')
    } else {
      console.log('VIP状态刷新完成（使用本地缓存）')
    }
  } catch (error) {
    console.error('VIP状态刷新异常:', error)
  } finally {
    isRefreshing.value = false
  }
}

// 当弹窗打开时自动刷新VIP状态
watch(() => props.visible, (newVisible) => {
  if (newVisible && userStore.isLoggedIn()) {
    refreshVipStatus()
  }
})

// 格式化剩余天数显示
const remainingDaysText = computed(() => {
  if (!vipStore.isVip || !vipStore.remainingDays.value) return ''
  const days = vipStore.remainingDays.value
  if (days <= 0) return '已过期'
  if (days <= 7) return `剩余 ${days} 天 (即将过期)`
  return `剩余 ${days} 天`
})

// VIP状态样式
const vipStatusClass = computed(() => {
  if (!vipStore.isVip.value) return 'status-free'
  if (vipStore.isExpired.value) return 'status-expired'
  if (vipStore.isExpiringSoon.value) return 'status-warning'
  return 'status-active'
})

const close = () => {
  emit('update:visible', false)
}

const handleOverlayClick = () => {
  close()
}
</script>

<style scoped>
.vip-account-overlay {
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

.vip-account-container {
  background: var(--card-bg, #ffffff);
  border-radius: var(--radius-xl, 16px);
  box-shadow: 0 20px 40px rgba(0, 0, 0, 0.15);
  width: 100%;
  max-width: 520px;
  margin: 20px;
  animation: slideUp 0.3s ease;
  max-height: 85vh;
  overflow-y: auto;
}

.dialog-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 24px 24px 0 24px;
  background: linear-gradient(135deg, var(--header-bg, #2c7a7b), #319795);
  color: white;
  border-radius: var(--radius-xl, 16px) var(--radius-xl, 16px) 0 0;
}

.dialog-title {
  margin: 0;
  font-size: 24px;
  font-weight: 700;
  display: flex;
  align-items: center;
  gap: 8px;
}

.dialog-title::before {
  content: '👑';
  font-size: 28px;
}

.close-btn {
  background: rgba(255, 255, 255, 0.2);
  border: none;
  font-size: 24px;
  color: white;
  cursor: pointer;
  width: 36px;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  transition: all 0.2s ease;
}

.close-btn:hover {
  background: rgba(255, 255, 255, 0.3);
  transform: scale(1.1);
}

.dialog-content {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

/* VIP区域样式 */
.vip-section {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.section-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 18px;
  font-weight: 600;
  color: var(--text-primary, #333);
  margin: 0;
}

.section-icon {
  font-size: 20px;
}

.refresh-btn {
  background: none;
  border: none;
  font-size: 16px;
  cursor: pointer;
  padding: 6px;
  border-radius: 50%;
  transition: all 0.2s ease;
  opacity: 0.6;
}

.refresh-btn:hover {
  opacity: 1;
  background: var(--bg-hover, #f5f5f5);
  transform: rotate(90deg);
}

.refreshing-indicator {
  font-size: 16px;
  opacity: 0.6;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.vip-status-card {
  display: flex;
  flex-direction: column;
  padding: 20px;
  border-radius: var(--radius-xl, 16px);
  border: 3px solid;
  transition: all 0.3s ease;
  position: relative;
  overflow: hidden;
  gap: 16px;
}

.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.vip-status-card::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: inherit;
  opacity: 0.1;
  z-index: -1;
}

.vip-status-card.status-free {
  border-color: #e2e8f0;
  background: linear-gradient(135deg, #f8fafc, #edf2f7);
}

.vip-status-card.status-active {
  border-color: #f6ad55;
  background: linear-gradient(135deg, #fffaf0, #fed7aa);
  box-shadow: 0 8px 32px rgba(246, 173, 85, 0.2);
}

.vip-status-card.status-warning {
  border-color: #f6ad55;
  background: linear-gradient(135deg, #fffbeb, #fde68a);
  box-shadow: 0 8px 32px rgba(246, 173, 85, 0.15);
}

.vip-status-card.status-expired {
  border-color: #fc8181;
  background: linear-gradient(135deg, #fef5e7, #fed7d7);
  box-shadow: 0 8px 32px rgba(252, 129, 129, 0.2);
}

.vip-main-info {
  display: flex;
  align-items: center;
  gap: 16px;
  flex: 1;
}

.vip-icon-large {
  font-size: 40px;
  width: 64px;
  height: 64px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.8);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.1);
}

.vip-details {
  flex: 1;
}

.vip-type {
  font-size: 22px;
  font-weight: 700;
  color: var(--text-primary, #333);
  margin-bottom: 4px;
}

.vip-status-text {
  font-size: 16px;
  color: var(--text-secondary, #666);
  margin-bottom: 6px;
}

.vip-expire-time {
  font-size: 14px;
  color: var(--text-secondary, #666);
  margin-bottom: 4px;
}

.vip-remaining {
  font-size: 14px;
  font-weight: 500;
}

.text-warning {
  color: #d69e2e;
}

.text-danger {
  color: #e53e3e;
}

.text-primary {
  color: var(--primary-color, #2c7a7b);
  font-weight: 600;
}

.upgrade-btn {
  padding: 12px 24px;
  background: linear-gradient(135deg, #2c7a7b, #319795);
  color: white;
  border: none;
  border-radius: var(--radius-lg, 12px);
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
  white-space: nowrap;
  box-shadow: 0 4px 12px rgba(44, 122, 123, 0.2);
}

.upgrade-btn:hover {
  background: linear-gradient(135deg, #319795, #2dd4bf);
  box-shadow: 0 6px 20px rgba(44, 122, 123, 0.3);
  transform: translateY(-2px);
}

/* 权益信息在卡片内的样式 */
.card-benefits {
  border-top: 1px solid rgba(255, 255, 255, 0.3);
  padding-top: 16px;
  margin-top: 4px;
}


.benefits-grid {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.benefit-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 0;
}

.benefit-icon {
  font-size: 20px;
  width: 28px;
  text-align: center;
}

.benefit-text {
  flex: 1;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.benefit-label {
  font-size: 14px;
  color: var(--text-secondary, #666);
  font-weight: 500;
}

.benefit-value {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary, #333);
}

.vip-history {
  background: rgba(255, 255, 255, 0.3);
  border-radius: var(--radius-lg, 12px);
  padding: 20px;
  border: 1px solid rgba(0, 0, 0, 0.05);
}

.history-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-primary, #333);
  margin: 0 0 16px 0;
  display: flex;
  align-items: center;
  gap: 8px;
}

.history-title::before {
  content: '📅';
  font-size: 18px;
}

.history-info {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.history-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  background: rgba(255, 255, 255, 0.6);
  border-radius: var(--radius-md, 8px);
}

.history-label {
  font-weight: 500;
  color: var(--text-secondary, #666);
}

.history-value {
  color: var(--text-primary, #333);
  font-weight: 500;
}

/* 未登录状态 */
.login-prompt {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  padding: 40px 20px;
  gap: 16px;
}

.prompt-icon {
  font-size: 48px;
  opacity: 0.6;
}

.prompt-text {
  font-size: 16px;
  color: var(--text-secondary, #666);
}

.login-btn {
  padding: 12px 24px;
  background: linear-gradient(135deg, #2c7a7b, #319795);
  color: white;
  border: none;
  border-radius: var(--radius-lg, 12px);
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
}

.login-btn:hover {
  background: linear-gradient(135deg, #319795, #2dd4bf);
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(44, 122, 123, 0.25);
}

.dialog-actions {
  display: flex;
  gap: 16px;
  justify-content: flex-end;
  margin-top: 8px;
  padding-top: 20px;
  border-top: 1px solid var(--border-color, #e2e8f0);
}

.action-btn {
  padding: 12px 24px;
  border: none;
  border-radius: var(--radius-lg, 12px);
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
  min-width: 100px;
}

.action-btn.secondary {
  background: var(--bg-secondary, #f8fafc);
  color: var(--text-secondary, #666);
  border: 2px solid var(--border-color, #e2e8f0);
}

.action-btn.secondary:hover {
  background: var(--bg-hover, #e2e8f0);
  color: var(--text-primary, #333);
}

.action-btn.primary {
  background: linear-gradient(135deg, #2c7a7b, #319795);
  color: white;
  box-shadow: 0 4px 12px rgba(44, 122, 123, 0.2);
}

.action-btn.primary:hover {
  background: linear-gradient(135deg, #319795, #2dd4bf);
  box-shadow: 0 6px 20px rgba(44, 122, 123, 0.3);
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
@media (max-width: 768px) {
  .vip-account-container {
    margin: 10px;
    max-width: none;
    max-height: 90vh;
  }
  
  .dialog-header, .dialog-content {
    padding-left: 16px;
    padding-right: 16px;
  }
  
  .dialog-title {
    font-size: 20px;
  }
  
  .card-header {
    flex-direction: column;
    gap: 12px;
    text-align: center;
  }
  
  .vip-main-info {
    flex-direction: column;
    text-align: center;
  }
  
  .vip-icon-large {
    width: 56px;
    height: 56px;
    font-size: 32px;
  }
  
  .upgrade-btn {
    width: 100%;
  }
  
  .dialog-actions {
    flex-direction: column;
    gap: 8px;
  }
  
  .action-btn {
    width: 100%;
  }
}

@media (max-width: 480px) {
  .vip-account-container {
    margin: 5px;
    max-height: 95vh;
    border-radius: 8px;
  }
  
  .dialog-header {
    padding: 16px 12px 0 12px;
    border-radius: 8px 8px 0 0;
  }
  
  .dialog-content {
    padding: 16px 12px;
  }
  
  .dialog-title {
    font-size: 18px;
  }
  
  .section-title {
    font-size: 16px;
  }
  
  .vip-status-card {
    padding: 16px;
  }
  
  .vip-type {
    font-size: 20px;
  }
  
  .vip-status-text {
    font-size: 14px;
  }
  
  .benefit-label,
  .benefit-value {
    font-size: 13px;
  }
  
  .login-prompt {
    padding: 30px 15px;
  }
  
  .prompt-text {
    font-size: 14px;
  }
}

/* 暗色模式支持 */
@media (prefers-color-scheme: dark) {
  .vip-account-container {
    --card-bg: #2d2d2d;
    --text-primary: #e6e6e6;
    --text-secondary: #999999;
    --bg-secondary: #3a3a3a;
    --bg-hover: #3d3d3d;
    --border-color: #3d3d3d;
  }
  
  .vip-status-card.status-free {
    border-color: #3d3d3d;
    background: linear-gradient(135deg, #2a2a2a, #333);
  }
  
  .vip-status-card.status-active {
    border-color: #f6ad55;
    background: linear-gradient(135deg, #2d2416, #3a2f1b);
  }
  
  .vip-status-card.status-warning {
    border-color: #f6ad55;
    background: linear-gradient(135deg, #332a1b, #3d2d1f);
  }
  
  .vip-status-card.status-expired {
    border-color: #fc8181;
    background: linear-gradient(135deg, #2d1b1b, #3d1f1f);
  }
  
  .vip-history, .history-item {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.1);
  }
  
  .card-benefits {
    border-top-color: rgba(255, 255, 255, 0.2);
  }
}
</style>