<template>
  <Transition name="message-slide" appear>
    <div class="message-bar" :class="[`message-bar--${type}`, 'message-bar--simple']">
      <!-- 主要内容 -->
      <div class="message-bar__content">
        <span class="message-bar__message">{{ message }}</span>
      </div>
      
      <!-- 进度条 -->
      <div class="message-bar__progress">
        <div class="message-bar__progress-bar" :class="`progress-bar--${type}`"></div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
defineProps<{
  message: string;
  type?: 'info' | 'warning' | 'error';
}>();

defineEmits<{
  close: [];
}>();
</script>

<style scoped>
/* 基础样式 */
.message-bar {
  position: fixed;
  top: 20px;
  left: 50%;
  transform: translateX(-50%);
  min-width: 200px;
  max-width: min(80vw, 320px);
  z-index: 9999;
  border-radius: 8px;
  overflow: hidden;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
  box-shadow: 
    0 4px 16px rgba(0, 0, 0, 0.12),
    0 2px 8px rgba(0, 0, 0, 0.08),
    0 1px 3px rgba(0, 0, 0, 0.06);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
}

/* 简约样式 */
.message-bar--simple {
  position: relative;
  background: rgba(255, 255, 255, 0.95);
  border: 1px solid rgba(255, 255, 255, 0.2);
}

/* 主要内容容器 */
.message-bar__content {
  padding: 12px 16px;
  position: relative;
  z-index: 1;
  text-align: center;
}

/* 文本内容 */
.message-bar__message {
  font-size: 14px;
  font-weight: 500;
  line-height: 1.4;
  letter-spacing: 0.01em;
  word-break: break-word;
  color: inherit;
}

/* 进度条 */
.message-bar__progress {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 3px;
  background: rgba(0, 0, 0, 0.1);
  overflow: hidden;
}

.message-bar__progress-bar {
  height: 100%;
  width: 100%;
  transform: translateX(-100%);
  transition: transform linear;
  border-radius: 2px;
}

/* 不同类型的主题色 */
.message-bar--info {
  color: #1e40af;
}

.message-bar--info .message-bar__progress-bar {
  background: linear-gradient(90deg, #3b82f6, #1d4ed8);
}

.message-bar--warning {
  color: #d97706;
}

.message-bar--warning .message-bar__progress-bar {
  background: linear-gradient(90deg, #f59e0b, #d97706);
}

.message-bar--error {
  color: #dc2626;
}

.message-bar--error .message-bar__progress-bar {
  background: linear-gradient(90deg, #ef4444, #dc2626);
}

/* 进度条动画 */
.progress-bar--info {
  animation: progress-info 2s linear forwards;
}

.progress-bar--warning {
  animation: progress-warning 3s linear forwards;
}

.progress-bar--error {
  animation: progress-error 3s linear forwards;
}

@keyframes progress-info {
  to { transform: translateX(0); }
}

@keyframes progress-warning {
  to { transform: translateX(0); }
}

@keyframes progress-error {
  to { transform: translateX(0); }
}

/* 入场动画 */
.message-slide-enter-active {
  transition: all 0.4s cubic-bezier(0.175, 0.885, 0.32, 1.275);
}

.message-slide-leave-active {
  transition: all 0.3s cubic-bezier(0.55, 0.085, 0.68, 0.53);
}

.message-slide-enter-from {
  transform: translateX(-50%) translateY(-100%) scale(0.8);
  opacity: 0;
}

.message-slide-leave-to {
  transform: translateX(-50%) translateY(-20px) scale(0.95);
  opacity: 0;
}

/* 暗色模式适配 */
@media (prefers-color-scheme: dark) {
  .message-bar--simple {
    background: rgba(28, 28, 30, 0.95);
    border-color: rgba(255, 255, 255, 0.1);
  }
  
  .message-bar__progress {
    background: rgba(255, 255, 255, 0.1);
  }
}

/* 响应式设计 */
@media (max-width: 480px) {
  .message-bar {
    min-width: 180px;
    max-width: calc(100vw - 32px);
    top: 16px;
    border-radius: 6px;
  }
  
  .message-bar__content {
    padding: 10px 12px;
  }
  
  .message-bar__message {
    font-size: 13px;
    line-height: 1.3;
  }
}

@media (max-width: 360px) {
  .message-bar {
    min-width: 160px;
    max-width: calc(100vw - 24px);
  }
  
  .message-bar__content {
    padding: 8px 10px;
  }
  
  .message-bar__message {
    font-size: 12px;
  }
}

/* 高对比度模式 */
@media (prefers-contrast: high) {
  .message-bar {
    border: 2px solid currentColor;
  }
}

/* 减少动画模式 */
@media (prefers-reduced-motion: reduce) {
  .message-slide-enter-active,
  .message-slide-leave-active {
    transition: opacity 0.2s ease;
  }
  
  .message-slide-enter-from,
  .message-slide-leave-to {
    transform: translateX(-50%);
    opacity: 0;
  }
  
  .message-bar__progress-bar {
    animation: none;
    transform: translateX(0);
  }
}
</style> 