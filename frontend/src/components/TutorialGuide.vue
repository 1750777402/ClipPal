<template>
  <div v-if="isVisible" class="tutorial-overlay">
    <!-- 背景遮罩 -->
    <div class="tutorial-backdrop" @click="handleSkip"></div>
    
    <!-- 高亮遮罩 -->
    <div v-if="highlightTarget" class="highlight-mask">
      <div class="highlight-hole" :style="highlightStyle"></div>
    </div>
    
    <!-- 引导卡片 -->
    <div class="tutorial-card" :class="`position-${currentStep.position}`" :style="cardPosition">
      <!-- 箭头指示器 -->
      <div v-if="currentStep.target !== 'body'" class="tutorial-arrow" :class="`arrow-${getArrowDirection()}`"></div>
      
      <div class="tutorial-header">
        <div class="tutorial-step-indicator">
          <span class="step-number">{{ currentStepIndex + 1 }}</span>
          <span class="step-total">/ {{ totalSteps }}</span>
        </div>
        <h3 class="tutorial-title">{{ currentStep.title }}</h3>
        <button class="tutorial-close" @click="handleSkip">✕</button>
      </div>
      
      <div class="tutorial-content">
        <p class="tutorial-description">{{ currentStep.description }}</p>
        
        <!-- 特殊内容区域 -->
        <div v-if="currentStep.id === 'shortcut_key'" class="demo-section">
          <div class="shortcut-demo">
            <kbd>Ctrl</kbd> <span class="plus">+</span> <kbd>`</kbd>
          </div>
          <p class="demo-tip">试试按下这个组合键！</p>
        </div>
        
        <div v-if="currentStep.id === 'tray_icon'" class="demo-section">
          <div class="tray-demo">
            <div class="tray-icon-demo">
              <div class="icon-bg">📋</div>
              <div class="click-animation"></div>
            </div>
            <p class="demo-tip">双击系统托盘图标</p>
          </div>
        </div>

        <div v-if="currentStep.id === 'copy_paste'" class="demo-section">
          <div class="action-demo">
            <div class="demo-card">
              <span>示例文本</span>
              <div class="double-click-hint">双击</div>
            </div>
            <div class="arrow-down">↓</div>
            <div class="result">自动复制并粘贴</div>
          </div>
        </div>
      </div>
      
      <div class="tutorial-footer">
        <div class="tutorial-progress">
          <div class="progress-bar">
            <div 
              class="progress-fill" 
              :style="{ width: `${progressPercentage}%` }"
            ></div>
          </div>
          <span class="progress-text">{{ currentStepIndex + 1 }} / {{ totalSteps }}</span>
        </div>
        
        <div class="tutorial-actions">
          <button 
            v-if="currentStepIndex > 0" 
            class="btn btn-secondary" 
            @click="previousStep"
          >
            ← 上一步
          </button>
          
          <button 
            class="btn btn-link" 
            @click="handleSkip"
          >
            跳过引导
          </button>
          
          <button 
            v-if="!isLastStep" 
            class="btn btn-primary" 
            @click="nextStep"
          >
            下一步 →
          </button>
          
          <button 
            v-else 
            class="btn btn-primary btn-complete" 
            @click="completeGuide"
          >
            <span>🎉</span> 完成引导
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, nextTick, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'

interface TutorialStep {
  id: string
  title: string
  description: string
  target: string
  position: string
}

interface Settings {
  auto_start: number
  max_records: number
  shortcut_key: string
  cloud_sync: number
  auto_paste: number
  tutorial_completed: number
}

const isVisible = ref(false)
const currentStepIndex = ref(0)
const highlightTarget = ref<HTMLElement | null>(null)
const highlightStyle = ref({})
const cardPosition = ref({})

// 引导步骤配置
const tutorialSteps = ref<TutorialStep[]>([
  {
    id: "welcome",
    title: "欢迎使用 ClipPal! 🎉",
    description: "ClipPal 是一个强大的剪贴板增强工具，让我们开始学习如何使用它吧！",
    target: "body",
    position: "center"
  },
  {
    id: "shortcut_key",
    title: "快捷键操作 ⌨️",
    description: "按下 Ctrl+` 可以快速打开/关闭剪贴板窗口。这是最常用的操作方式！",
    target: ".panel-header",
    position: "bottom"
  },
  {
    id: "tray_icon",
    title: "托盘图标 📌",
    description: "双击系统托盘的 ClipPal 图标也可以打开窗口。右键点击可以访问设置和退出选项。",
    target: "body",
    position: "center"
  },
  {
    id: "copy_paste",
    title: "复制与粘贴 📋",
    description: "双击任意剪贴板记录即可复制并自动粘贴到当前应用。右侧菜单可以进行更多操作。",
    target: ".clip-list",
    position: "left"
  },
  {
    id: "auto_paste",
    title: "自动粘贴功能 🚀",
    description: "开启自动粘贴后，选择剪贴板记录会自动粘贴到之前的活动窗口，大大提升工作效率！",
    target: ".settings-button",
    position: "bottom"
  },
  {
    id: "settings",
    title: "个性化设置 ⚙️",
    description: "在设置中可以自定义快捷键、开启开机自启、调整最大记录数等选项，让 ClipPal 更适合你的使用习惯。",
    target: ".settings-button",
    position: "bottom"
  },
  {
    id: "complete",
    title: "引导完成! 🎊",
    description: "恭喜！您已经学会了 ClipPal 的基本使用方法。现在开始享受高效的剪贴板管理体验吧！",
    target: "body",
    position: "center"
  }
])

const currentStep = computed(() => {
  return tutorialSteps.value[currentStepIndex.value] || {}
})

const totalSteps = computed(() => tutorialSteps.value.length)

const isLastStep = computed(() => currentStepIndex.value === totalSteps.value - 1)

const progressPercentage = computed(() => {
  return ((currentStepIndex.value + 1) / totalSteps.value) * 100
})

// 检查是否需要显示引导
const checkShouldShowTutorial = async () => {
  try {
    const settings = await invoke<Settings>('load_settings')
    if (settings.tutorial_completed === 0) {
      isVisible.value = true
      updateHighlight()
    }
  } catch (error) {
    console.error('检查引导状态失败:', error)
  }
}

// 更新高亮区域和卡片位置
const updateHighlight = async () => {
  await nextTick()
  
  const step = currentStep.value
  if (!step.target || step.target === 'body') {
    highlightTarget.value = null
    cardPosition.value = {}
    return
  }
  
  const target = document.querySelector(step.target) as HTMLElement
  if (target) {
    highlightTarget.value = target
    const rect = target.getBoundingClientRect()
    
    // 高亮样式
    highlightStyle.value = {
      top: `${rect.top - 8}px`,
      left: `${rect.left - 8}px`,
      width: `${rect.width + 16}px`,
      height: `${rect.height + 16}px`,
    }
    
    // 计算卡片位置
    const cardRect = calculateCardPosition(rect, step.position)
    cardPosition.value = {
      top: `${cardRect.top}px`,
      left: `${cardRect.left}px`,
      transform: 'none'
    }
  }
}

// 计算卡片位置
const calculateCardPosition = (targetRect: DOMRect, position: string) => {
  const cardWidth = 400
  const cardHeight = 300
  const gap = 20
  
  let top = 0
  let left = 0
  
  switch (position) {
    case 'top':
      top = targetRect.top - cardHeight - gap
      left = targetRect.left + (targetRect.width - cardWidth) / 2
      break
    case 'bottom':
      top = targetRect.bottom + gap
      left = targetRect.left + (targetRect.width - cardWidth) / 2
      break
    case 'left':
      top = targetRect.top + (targetRect.height - cardHeight) / 2
      left = targetRect.left - cardWidth - gap
      break
    case 'right':
      top = targetRect.top + (targetRect.height - cardHeight) / 2
      left = targetRect.right + gap
      break
    default:
      return { top: 0, left: 0 }
  }
  
  // 确保卡片在视窗内
  const windowWidth = window.innerWidth
  const windowHeight = window.innerHeight
  
  left = Math.max(gap, Math.min(left, windowWidth - cardWidth - gap))
  top = Math.max(gap, Math.min(top, windowHeight - cardHeight - gap))
  
  return { top, left }
}

// 获取箭头方向
const getArrowDirection = () => {
  const position = currentStep.value.position
  switch (position) {
    case 'top': return 'down'
    case 'bottom': return 'up'
    case 'left': return 'right'
    case 'right': return 'left'
    default: return 'up'
  }
}

// 下一步
const nextStep = () => {
  if (currentStepIndex.value < totalSteps.value - 1) {
    currentStepIndex.value++
    updateHighlight()
  }
}

// 上一步
const previousStep = () => {
  if (currentStepIndex.value > 0) {
    currentStepIndex.value--
    updateHighlight()
  }
}

// 完成引导
const completeGuide = async () => {
  try {
    const currentSettings = await invoke<Settings>('load_settings')
    currentSettings.tutorial_completed = 1
    await invoke('save_settings', { settings: currentSettings })
    isVisible.value = false
  } catch (error) {
    console.error('完成引导失败:', error)
  }
}

// 跳过引导
const handleSkip = async () => {
  await completeGuide()
}

// 重置引导（开发用）
const resetTutorial = async () => {
  try {
    const currentSettings = await invoke<Settings>('load_settings')
    currentSettings.tutorial_completed = 0
    await invoke('save_settings', { settings: currentSettings })
    await checkShouldShowTutorial()
  } catch (error) {
    console.error('重置引导失败:', error)
  }
}

// 监听步骤变化
watch(currentStepIndex, () => {
  updateHighlight()
})

// 监听窗口大小变化
const handleResize = () => {
  updateHighlight()
}

// 组件挂载时检查是否需要显示引导
onMounted(() => {
  checkShouldShowTutorial()
  window.addEventListener('resize', handleResize)
})

// 暴露方法给外部调用
defineExpose({
  checkShouldShowTutorial,
  resetTutorial
})
</script>

<style scoped>
.tutorial-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: 9999;
  pointer-events: auto;
}

.tutorial-backdrop {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: blur(3px);
  animation: fadeIn 0.3s ease;
}

.highlight-mask {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  pointer-events: none;
  z-index: 2;
}

.highlight-hole {
  position: absolute;
  border: 3px solid #4285f4;
  border-radius: 12px;
  box-shadow: 
    0 0 0 4px rgba(66, 133, 244, 0.3),
    0 0 0 9999px rgba(0, 0, 0, 0.4);
  transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1);
  animation: pulse 2s infinite;
}

@keyframes pulse {
  0%, 100% {
    box-shadow: 
      0 0 0 4px rgba(66, 133, 244, 0.3),
      0 0 0 9999px rgba(0, 0, 0, 0.4);
  }
  50% {
    box-shadow: 
      0 0 0 8px rgba(66, 133, 244, 0.5),
      0 0 0 9999px rgba(0, 0, 0, 0.4);
  }
}

.tutorial-card {
  position: absolute;
  background: white;
  border-radius: 16px;
  box-shadow: 
    0 20px 40px rgba(0, 0, 0, 0.2),
    0 0 0 1px rgba(0, 0, 0, 0.05);
  min-width: 360px;
  max-width: 400px;
  overflow: hidden;
  animation: slideIn 0.4s cubic-bezier(0.4, 0, 0.2, 1);
  z-index: 3;
}

@keyframes slideIn {
  from {
    opacity: 0;
    transform: scale(0.9) translateY(20px);
  }
  to {
    opacity: 1;
    transform: scale(1) translateY(0);
  }
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

/* 箭头指示器 */
.tutorial-arrow {
  position: absolute;
  width: 0;
  height: 0;
  z-index: 4;
}

.arrow-up {
  bottom: -12px;
  left: 50%;
  transform: translateX(-50%);
  border-left: 12px solid transparent;
  border-right: 12px solid transparent;
  border-top: 12px solid white;
  filter: drop-shadow(0 4px 8px rgba(0, 0, 0, 0.1));
}

.arrow-down {
  top: -12px;
  left: 50%;
  transform: translateX(-50%);
  border-left: 12px solid transparent;
  border-right: 12px solid transparent;
  border-bottom: 12px solid white;
  filter: drop-shadow(0 -4px 8px rgba(0, 0, 0, 0.1));
}

.arrow-left {
  right: -12px;
  top: 50%;
  transform: translateY(-50%);
  border-top: 12px solid transparent;
  border-bottom: 12px solid transparent;
  border-left: 12px solid white;
  filter: drop-shadow(4px 0 8px rgba(0, 0, 0, 0.1));
}

.arrow-right {
  left: -12px;
  top: 50%;
  transform: translateY(-50%);
  border-top: 12px solid transparent;
  border-bottom: 12px solid transparent;
  border-right: 12px solid white;
  filter: drop-shadow(-4px 0 8px rgba(0, 0, 0, 0.1));
}

/* 默认定位样式 */
.position-center {
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
}

.tutorial-header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 20px 24px 16px;
  background: linear-gradient(135deg, #4285f4 0%, #34a853 100%);
  color: white;
}

.tutorial-step-indicator {
  display: flex;
  align-items: center;
  background: rgba(255, 255, 255, 0.2);
  border-radius: 12px;
  padding: 4px 8px;
  font-size: 12px;
  font-weight: 600;
}

.step-number {
  font-size: 14px;
}

.step-total {
  opacity: 0.8;
}

.tutorial-title {
  flex: 1;
  margin: 0;
  font-size: 18px;
  font-weight: 600;
}

.tutorial-close {
  background: rgba(255, 255, 255, 0.2);
  border: none;
  border-radius: 50%;
  width: 32px;
  height: 32px;
  color: white;
  cursor: pointer;
  font-size: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s ease;
}

.tutorial-close:hover {
  background: rgba(255, 255, 255, 0.3);
  transform: scale(1.1);
}

.tutorial-content {
  padding: 24px;
}

.tutorial-description {
  margin: 0 0 20px 0;
  line-height: 1.6;
  color: #444;
  font-size: 15px;
}

.demo-section {
  margin: 20px 0;
  text-align: center;
}

.shortcut-demo {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 20px;
  background: linear-gradient(135deg, #f8f9fa 0%, #e9ecef 100%);
  border-radius: 12px;
  border: 2px solid #4285f4;
  position: relative;
  overflow: hidden;
}

.shortcut-demo::before {
  content: '';
  position: absolute;
  top: 0;
  left: -100%;
  width: 100%;
  height: 100%;
  background: linear-gradient(90deg, transparent, rgba(66, 133, 244, 0.1), transparent);
  animation: shimmer 2s infinite;
}

@keyframes shimmer {
  0% { left: -100%; }
  100% { left: 100%; }
}

.shortcut-demo kbd {
  background: linear-gradient(135deg, #fff 0%, #f8f9fa 100%);
  border: 2px solid #ddd;
  border-radius: 8px;
  padding: 8px 12px;
  font-family: 'Courier New', monospace;
  font-size: 16px;
  font-weight: bold;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
  position: relative;
}

.plus {
  font-size: 18px;
  font-weight: bold;
  color: #4285f4;
}

.tray-demo {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  padding: 20px;
  background: linear-gradient(135deg, #f8f9fa 0%, #e9ecef 100%);
  border-radius: 12px;
  border: 2px solid #4285f4;
}

.tray-icon-demo {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
}

.icon-bg {
  font-size: 32px;
  padding: 12px;
  background: linear-gradient(135deg, #fff 0%, #f8f9fa 100%);
  border-radius: 50%;
  box-shadow: 0 4px 8px rgba(0, 0, 0, 0.15);
  position: relative;
  z-index: 2;
}

.click-animation {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 60px;
  height: 60px;
  border: 2px solid #4285f4;
  border-radius: 50%;
  animation: clickPulse 1.5s infinite;
}

@keyframes clickPulse {
  0% {
    transform: translate(-50%, -50%) scale(0.8);
    opacity: 1;
  }
  100% {
    transform: translate(-50%, -50%) scale(1.5);
    opacity: 0;
  }
}

.action-demo {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  padding: 20px;
  background: linear-gradient(135deg, #f8f9fa 0%, #e9ecef 100%);
  border-radius: 12px;
  border: 2px solid #4285f4;
}

.demo-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  background: white;
  border-radius: 8px;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
  position: relative;
  cursor: pointer;
  transition: all 0.3s ease;
}

.demo-card:hover {
  transform: scale(1.05);
  box-shadow: 0 4px 8px rgba(0, 0, 0, 0.15);
}

.double-click-hint {
  background: #4285f4;
  color: white;
  padding: 4px 8px;
  border-radius: 12px;
  font-size: 12px;
  font-weight: bold;
  animation: bounce 1s infinite;
}

@keyframes bounce {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(-4px); }
}

.arrow-down {
  font-size: 24px;
  color: #4285f4;
  animation: moveDown 1s infinite;
}

@keyframes moveDown {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(4px); }
}

.result {
  padding: 8px 16px;
  background: #34a853;
  color: white;
  border-radius: 8px;
  font-weight: bold;
  font-size: 14px;
}

.demo-tip {
  margin: 8px 0 0 0;
  font-size: 13px;
  color: #666;
  font-style: italic;
}

.tutorial-footer {
  padding: 16px 24px 24px;
  background: #fafafa;
  border-top: 1px solid #e9ecef;
}

.tutorial-progress {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 20px;
}

.progress-bar {
  flex: 1;
  height: 6px;
  background: #e9ecef;
  border-radius: 3px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: linear-gradient(90deg, #4285f4 0%, #34a853 100%);
  transition: width 0.4s cubic-bezier(0.4, 0, 0.2, 1);
  border-radius: 3px;
}

.progress-text {
  font-size: 12px;
  color: #666;
  font-weight: 600;
  min-width: 40px;
}

.tutorial-actions {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
}

.btn {
  padding: 10px 20px;
  border: none;
  border-radius: 8px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
  text-decoration: none;
  display: flex;
  align-items: center;
  gap: 4px;
}

.btn-primary {
  background: linear-gradient(135deg, #4285f4 0%, #34a853 100%);
  color: white;
}

.btn-primary:hover {
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(66, 133, 244, 0.3);
}

.btn-complete {
  background: linear-gradient(135deg, #ff6b6b 0%, #feca57 100%);
}

.btn-secondary {
  background: #f8f9fa;
  color: #666;
  border: 1px solid #ddd;
}

.btn-secondary:hover {
  background: #e9ecef;
  transform: translateY(-1px);
}

.btn-link {
  background: none;
  color: #666;
  padding: 10px 16px;
  font-size: 13px;
}

.btn-link:hover {
  color: #333;
  background: rgba(0, 0, 0, 0.05);
}
</style> 