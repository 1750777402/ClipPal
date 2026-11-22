<template>
  <div class="app-container" :class="responsiveClasses">
    <ScrollContainer />
    <MessageBar v-if="messageBar.visible" :message="messageBar.message" :type="messageBar.type"
      @mouseenter="onMessageBarEnter" @mouseleave="onMessageBarLeave" />
    <TutorialGuide ref="tutorialGuideRef" />
    <UpdateDialog v-model="showUpdateDialog" ref="updateDialogRef" />
  </div>
</template>

<script setup lang="ts">
import { ref, provide, computed, onMounted, onUnmounted } from 'vue';
import MessageBar from './components/MessageBar.vue';
import ScrollContainer from './components/ScrollContainer.vue';
import TutorialGuide from './components/TutorialGuide.vue';
import UpdateDialog from './components/UpdateDialog.vue';
import { useBreakpoint, generateResponsiveClasses } from './utils/responsive';
import { setErrorHandler, ErrorSeverity, getFriendlyErrorMessage } from './utils/api';

const messageBar = ref({ visible: false, message: '', type: 'info' as 'info' | 'warning' | 'error' });
const showUpdateDialog = ref(false);
const updateDialogRef = ref<InstanceType<typeof UpdateDialog> | null>(null);
let closeTimer: ReturnType<typeof setTimeout> | null = null;
let updateCheckTimer: ReturnType<typeof setTimeout> | null = null;
let isHovering = false;

// 响应式功能
const breakpoint = useBreakpoint();
const responsiveClasses = computed(() => generateResponsiveClasses(breakpoint));

/** 展示一条统一消息，替换已有关闭计时；用户悬停时保留提示。 */
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

/** 鼠标进入提示条时暂停自动关闭，便于读完消息。 */
function onMessageBarEnter() {
  isHovering = true;
  if (closeTimer) clearTimeout(closeTimer);
}

/** 鼠标离开后延迟关闭，避免提示条立刻消失。 */
function onMessageBarLeave() {
  isHovering = false;
  if (closeTimer) clearTimeout(closeTimer);
  closeTimer = setTimeout(() => {
    if (!isHovering) messageBar.value.visible = false;
  }, 1000);
}


provide('showMessageBar', showMessageBar);

/** 手动检查优先：取消尚未开始的启动检查，并交给更新弹窗显示进度。 */
function handleManualUpdateCheck() {
  if (updateCheckTimer) {
    clearTimeout(updateCheckTimer);
    updateCheckTimer = null;
  }
  void updateDialogRef.value?.checkUpdate();
}

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

  // 监听手动检查更新事件
  window.addEventListener('check-update', handleManualUpdateCheck);

  // 等界面和 IPC 通道准备完成后静默检查更新，失败不会影响应用启动。
  updateCheckTimer = setTimeout(() => {
    updateCheckTimer = null;
    void updateDialogRef.value?.checkUpdate(false);
  }, 5000);


});



// 清理事件监听器
onUnmounted(() => {
  // 清理定时器
  if (closeTimer) {
    clearTimeout(closeTimer);
    closeTimer = null;
  }
  if (updateCheckTimer) {
    clearTimeout(updateCheckTimer);
    updateCheckTimer = null;
  }

  window.removeEventListener('check-update', handleManualUpdateCheck);


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
