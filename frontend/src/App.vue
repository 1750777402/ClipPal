<template>
  <div class="app-container" :class="responsiveClasses">
    <ScrollContainer />
    <MessageBar v-if="messageBar.visible" :message="messageBar.message" :type="messageBar.type"
      @mouseenter="onMessageBarEnter" @mouseleave="onMessageBarLeave" />
    <TutorialGuide ref="tutorialGuideRef" />
  </div>
</template>

<script setup lang="ts">
import { ref, provide, computed, onMounted } from 'vue';
import MessageBar from './components/MessageBar.vue';
import ScrollContainer from './components/ScrollContainer.vue';
import TutorialGuide from './components/TutorialGuide.vue';
import { useBreakpoint, generateResponsiveClasses } from './utils/responsive';
import { setErrorHandler, ErrorSeverity, getFriendlyErrorMessage } from './utils/api';

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

// 设置全局错误处理器
onMounted(() => {
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