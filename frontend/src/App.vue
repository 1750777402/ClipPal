<template>
  <div class="app-container" :class="responsiveClasses">
    <ScrollContainer />
    <MessageBar v-if="messageBar.visible" :message="messageBar.message" :type="messageBar.type"
      @mouseenter="onMessageBarEnter" @mouseleave="onMessageBarLeave" />
    <TutorialGuide ref="tutorialGuideRef" />
  </div>
</template>

<script setup lang="ts">
import { ref, provide, computed, onMounted, onUnmounted } from 'vue';
import { listen } from '@tauri-apps/api/event';
import MessageBar from './components/MessageBar.vue';
import ScrollContainer from './components/ScrollContainer.vue';
import TutorialGuide from './components/TutorialGuide.vue';
import { useBreakpoint, generateResponsiveClasses } from './utils/responsive';
import { setErrorHandler, ErrorSeverity, getFriendlyErrorMessage } from './utils/api';
import { useUserStore } from './utils/userStore';
import { useVipStore } from './utils/vipStore';

const messageBar = ref({ visible: false, message: '', type: 'info' as 'info' | 'warning' | 'error' });
let closeTimer: ReturnType<typeof setTimeout> | null = null;
let isHovering = false;

// 响应式功能
const breakpoint = useBreakpoint();
const responsiveClasses = computed(() => generateResponsiveClasses(breakpoint));

function showMessageBar(message: string, type: 'info' | 'warning' | 'error' = 'info') {
  messageBar.value.message = message;
  messageBar.value.type = type;
  messageBar.value.visible = true;
  if (closeTimer) clearTimeout(closeTimer);
  
  // 根据消息类型设置不同的显示时间
  const displayTime = type === 'error' ? 3000 : type === 'warning' ? 3000 : 2000;
  
  closeTimer = setTimeout(() => {
    if (!isHovering) messageBar.value.visible = false;
  }, displayTime);
}

function onMessageBarEnter() {
  isHovering = true;
  if (closeTimer) clearTimeout(closeTimer);
}

function onMessageBarLeave() {
  isHovering = false;
  if (closeTimer) clearTimeout(closeTimer);
  closeTimer = setTimeout(() => {
    if (!isHovering) messageBar.value.visible = false;
  }, 1000);
}


provide('showMessageBar', showMessageBar);

const userStore = useUserStore();
const vipStore = useVipStore();
let authExpiredListener: (() => void) | null = null;
let authClearedListener: (() => void) | null = null;
let cloudSyncDisabledListener: (() => void) | null = null;

// 设置全局错误处理器和事件监听
onMounted(async () => {
  setErrorHandler((error: string, severity: ErrorSeverity, command: string) => {
    // 根据错误严重程度决定是否显示
    if (severity === ErrorSeverity.SILENT) return;
    
    // 获取友好的错误消息
    const friendlyMessage = getFriendlyErrorMessage(error, command);
    
    // 根据严重程度选择显示方式
    const messageType = severity === ErrorSeverity.CRITICAL ? 'error' 
                      : severity === ErrorSeverity.WARNING ? 'warning' 
                      : 'info';
    
    // 显示消息
    showMessageBar(friendlyMessage, messageType);
  });

  // 初始化VIP状态
  try {
    await vipStore.initialize();
  } catch (error) {
    console.error('VIP状态初始化失败:', error);
  }

  // 监听认证过期事件
  authExpiredListener = await listen('auth-expired', () => {
    console.log('收到认证过期事件');
    userStore.clearLoginState();
    vipStore.clearVipState(); // 清除VIP状态
    showMessageBar('登录已过期，请重新登录', 'warning');
  });

  // 监听认证清除事件
  authClearedListener = await listen('auth-cleared', () => {
    console.log('收到认证清除事件');
    userStore.clearLoginState();
    vipStore.clearVipState(); // 清除VIP状态
  });

  // 监听云同步禁用事件
  cloudSyncDisabledListener = await listen('cloud-sync-disabled', () => {
    console.log('收到云同步禁用事件');
    showMessageBar('云同步功能已关闭', 'info');
    // TODO: 更新前端云同步状态
  });
});

// 清理事件监听器
onUnmounted(() => {
  if (authExpiredListener) {
    authExpiredListener();
  }
  if (authClearedListener) {
    authClearedListener();
  }
  if (cloudSyncDisabledListener) {
    cloudSyncDisabledListener();
  }
});
</script>

<script lang="ts">
import { defineComponent } from 'vue';
export default defineComponent({});
</script>

<style scoped>
.app-container {
  width: 100%;
  height: 100vh;
  overflow: hidden;
}
</style>