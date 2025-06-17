<template>
  <ScrollContainer />
  <MessageBar v-if="messageBar.visible" :message="messageBar.message" :type="messageBar.type" />
</template>

<script setup lang="ts">
import { ref, provide } from 'vue';
import MessageBar from './components/MessageBar.vue';
import ScrollContainer from './components/ScrollContainer.vue';

const messageBar = ref({ visible: false, message: '', type: 'success' as 'success' | 'error' });

function showMessageBar(message: string, type: 'success' | 'error' = 'success') {
  messageBar.value.message = message;
  messageBar.value.type = type;
  messageBar.value.visible = true;
  setTimeout(() => {
    messageBar.value.visible = false;
  }, 2000);
}

provide('showMessageBar', showMessageBar);
</script>

<script lang="ts">
import { defineComponent } from 'vue';
export default defineComponent({});
</script>

<style scoped>
/* message-bar 样式已迁移到 MessageBar.vue */
</style>