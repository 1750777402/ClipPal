<template>
  <div class="message-bar" :class="type">
    {{ message }}
  </div>
</template>

<script setup lang="ts">
defineProps<{
  message: string;
  type?: 'success' | 'error';
}>();
</script>

<style scoped>
.message-bar {
  position: fixed;
  top: 32px;
  left: 50%;
  transform: translateX(-50%);
  min-width: 200px;
  max-width: 90vw;
  max-height: 220px;
  padding: 12px 32px;
  border-radius: 10px;
  font-size: 15px;
  color: #fff;
  z-index: 9999;
  box-shadow: 0 4px 16px rgba(44, 122, 123, 0.12);
  background: var(--primary-color, #2c7a7b);
  transition: all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
  opacity: 0.98;
  text-align: left;
  word-break: break-all;
  overflow-y: auto;
  line-height: 1.7;
  scrollbar-width: thin;
  scrollbar-color: #ffd6d6 transparent;
}

.message-bar.error {
  background: #e53e3e;
}

.message-bar.success {
  background: #2c7a7b;
}

.message-bar::-webkit-scrollbar {
  width: 6px;
  background: transparent;
}
.message-bar::-webkit-scrollbar-thumb {
  background: #ffd6d6;
  border-radius: 6px;
}
.message-bar::-webkit-scrollbar-thumb:hover {
  background: #ffb3b3;
}
.message-bar::-webkit-scrollbar-corner {
  background: transparent;
}
</style> 