<template>
  <div class="app-container">
    <ScrollContainer />
  </div>
</template>

<script setup lang="ts">
import ScrollContainer from '../src/components/ScrollContainer.vue'
import { onMounted } from 'vue';

onMounted(() => {
  window.addEventListener('contextmenu', (e) => {
    e.preventDefault();
  });
});

</script>

<style scoped>
.app-container {
  width: 100%;
  height: 100%;
  overflow: hidden;
  user-select: none
}

</style>