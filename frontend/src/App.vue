<template>
  <div class="app-container" :class="responsiveClasses">
    <ScrollContainer />
    <MessageBar v-if="messageBar.visible" :message="messageBar.message" :type="messageBar.type"
      @mouseenter="onMessageBarEnter" @mouseleave="onMessageBarLeave" />
    <TutorialGuide ref="tutorialGuideRef" />
  </div>
</template>

<script setup lang="ts">
import { ref, provide, computed } from 'vue';
import MessageBar from './components/MessageBar.vue';
import ScrollContainer from './components/ScrollContainer.vue';
import TutorialGuide from './components/TutorialGuide.vue';
import { useBreakpoint, generateResponsiveClasses } from './utils/responsive';

const messageBar = ref({ visible: false, message: '', type: 'success' as 'success' | 'error' });
let closeTimer: ReturnType<typeof setTimeout> | null = null;
let isHovering = false;

// 响应式功能
const breakpoint = useBreakpoint();
const responsiveClasses = computed(() => generateResponsiveClasses(breakpoint));

function showMessageBar(message: string, type: 'success' | 'error' = 'success') {
  messageBar.value.message = message;
  messageBar.value.type = type;
  messageBar.value.visible = true;
  if (closeTimer) clearTimeout(closeTimer);
  closeTimer = setTimeout(() => {
    if (!isHovering) messageBar.value.visible = false;
  }, 2000);
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