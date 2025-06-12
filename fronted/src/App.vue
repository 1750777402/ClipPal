<template>
  <div class="app-container">
    <ScrollContainer class="scroll-container"/>
  </div>
</template>

<script setup lang="ts">
import ScrollContainer from '../src/components/ScrollContainer.vue'
</script>

<style scoped>
.app-container {
  height: 100vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
</style>