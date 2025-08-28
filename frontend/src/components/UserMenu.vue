<template>
  <div v-if="visible" class="user-menu-overlay" @click="close">
    <div class="user-menu" @click.stop>
      <div class="user-info">
        <div class="user-avatar">
          {{ userInfo?.account?.charAt(0)?.toUpperCase() || 'U' }}
        </div>
        <div class="user-details">
          <div class="user-name">{{ userInfo?.account || '用户' }}</div>
          <div class="user-email">{{ userInfo?.email || '未设置邮箱' }}</div>
        </div>
      </div>
      
      <div class="menu-divider"></div>
      
      <div class="menu-items">
        <button class="menu-item" @click="handleUserInfo" type="button">
          <span class="menu-icon">👤</span>
          <span>个人信息</span>
          <span class="menu-arrow">›</span>
        </button>
        
        <button class="menu-item" @click="handleVipAccount" type="button">
          <span class="menu-icon">👑</span>
          <span>VIP账户</span>
          <span class="menu-arrow">›</span>
        </button>
        
        <div class="menu-divider"></div>
        
        <button class="menu-item logout-item" @click="handleLogout" type="button">
          <span class="menu-icon">🚪</span>
          <span>退出登录</span>
        </button>
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
  (e: 'logout'): void
  (e: 'user-info'): void
  (e: 'vip-account'): void
}

defineProps<Props>()
const emit = defineEmits<Emits>()

const close = () => {
  emit('update:visible', false)
}

const handleLogout = () => {
  emit('logout')
  close()
}

const handleUserInfo = () => {
  emit('user-info')
  close()
}

const handleVipAccount = () => {
  emit('vip-account')
  close()
}
</script>

<style scoped>
.user-menu-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.1);
  z-index: 1000;
  display: flex;
  align-items: flex-start;
  justify-content: flex-end;
  padding: 60px 20px 20px;
  animation: fadeIn 0.2s ease;
}

.user-menu {
  background: var(--card-bg, #ffffff);
  border-radius: var(--radius-lg, 12px);
  box-shadow: 0 10px 25px rgba(0, 0, 0, 0.15);
  width: 280px;
  padding: 0;
  animation: slideDown 0.3s ease;
  border: 1px solid var(--border-color, #e2e8f0);
}

.user-info {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 20px;
  background: linear-gradient(135deg, var(--header-bg, #2c7a7b), #319795);
  border-radius: var(--radius-lg, 12px) var(--radius-lg, 12px) 0 0;
  color: white;
}

.user-avatar {
  width: 48px;
  height: 48px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.2);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 20px;
  font-weight: bold;
  color: white;
}

.user-details {
  flex: 1;
}

.user-name {
  font-size: 16px;
  font-weight: 600;
  margin-bottom: 4px;
}

.user-email {
  font-size: 12px;
  opacity: 0.8;
}

.menu-divider {
  height: 1px;
  background: var(--border-color, #e2e8f0);
  margin: 0;
}

.menu-items {
  padding: 8px 0;
}

.menu-item {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  padding: 12px 20px;
  border: none;
  background: none;
  color: var(--text-primary, #333);
  font-size: 14px;
  cursor: pointer;
  transition: background-color 0.2s ease;
  text-align: left;
  position: relative;
}

.menu-item > span:nth-child(2) {
  flex: 1;
}

.menu-arrow {
  margin-left: auto;
  opacity: 0.5;
  font-size: 16px;
  transition: opacity 0.2s ease;
}

.menu-item:hover {
  background: var(--bg-hover, #f7fafc);
}

.menu-item:hover .menu-arrow {
  opacity: 1;
}

.menu-icon {
  font-size: 16px;
  width: 20px;
  text-align: center;
}

.logout-item {
  color: var(--text-danger, #e53e3e);
}

.logout-item:hover {
  background: var(--bg-danger-light, #fed7d7);
}

@keyframes fadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

@keyframes slideDown {
  from {
    opacity: 0;
    transform: translateY(-10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

/* 响应式适配 */
@media (max-width: 480px) {
  .user-menu-overlay {
    padding: 60px 15px 20px;
  }
  
  .user-menu {
    width: 100%;
    max-width: 320px;
  }
  
  .user-info {
    padding: 16px;
  }
  
  .user-avatar {
    width: 40px;
    height: 40px;
    font-size: 18px;
  }
  
  .menu-item {
    padding: 10px 16px;
  }
}

/* 暗色模式支持 */
@media (prefers-color-scheme: dark) {
  .user-menu {
    --card-bg: #2d2d2d;
    --text-primary: #e6e6e6;
    --border-color: #3d3d3d;
    --bg-hover: #3d3d3d;
    --text-danger: #fc8181;
    --bg-danger-light: #3d2d2d;
  }
}
</style>