<template>
  <div v-if="isVisible" class="tutorial-overlay">
    <!-- 背景遮罩 -->
    <div class="tutorial-backdrop" @click="handleSkip"></div>
    
    <!-- 高亮遮罩 -->
    <div v-if="highlightTarget" class="highlight-mask">
      <div class="highlight-hole" :style="highlightStyle"></div>
    </div>
    
    <!-- 引导卡片 -->
    <div class="tutorial-card" :class="`position-${actualCardPosition}`" :style="cardPosition">
      <!-- 箭头指示器 -->
      <div v-if="currentStep.target !== 'body' && getArrowDirection() !== 'none'" class="tutorial-arrow" :class="`arrow-${getArrowDirection()}`"></div>
      
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

        <div v-if="currentStep.id === 'auto_paste'" class="demo-section">
          <div class="auto-paste-warning">
            <div class="warning-icon">⚠️</div>
            <div class="warning-content">
              <div class="warning-title">使用提醒</div>
              <div class="warning-text">
                某些应用可能自定义了Ctrl+V快捷键，可根据实际使用情况选择是否开启自动粘贴。
              </div>
            </div>
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
import { ref, computed, onMounted, onBeforeUnmount, nextTick, watch } from 'vue'
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
    description: "开启自动粘贴后，选择剪贴板记录会自动粘贴到之前的活动窗口，大大提升工作效率！注意：如果目标应用自定义了Ctrl+V快捷键（如开发工具、终端、游戏等），可能会触发意外操作，建议在这些应用中关闭自动粘贴。",
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

// 添加实际位置状态跟踪
const actualCardPosition = ref('center')

// 计算卡片位置
const calculateCardPosition = (targetRect: DOMRect, preferredPosition: string) => {
  // 动态获取卡片实际尺寸
  const cardElement = document.querySelector('.tutorial-card') as HTMLElement
  const cardWidth = cardElement ? Math.min(cardElement.offsetWidth, 420) : 420
  const cardHeight = cardElement ? Math.min(cardElement.offsetHeight, 500) : 400
  
  const gap = 24
  const windowWidth = window.innerWidth
  const windowHeight = window.innerHeight
  const scrollTop = window.scrollY || document.documentElement.scrollTop
  const scrollLeft = window.scrollX || document.documentElement.scrollLeft
  
  // 可用空间计算
  const spaces = {
    top: targetRect.top - gap - cardHeight,
    bottom: windowHeight - targetRect.bottom - gap - cardHeight,
    left: targetRect.left - gap - cardWidth,
    right: windowWidth - targetRect.right - gap - cardWidth
  }
  
  // 位置优先级列表
  const positionPriority = {
    'top': ['top', 'bottom', 'right', 'left', 'center'],
    'bottom': ['bottom', 'top', 'right', 'left', 'center'],
    'left': ['left', 'right', 'bottom', 'top', 'center'],
    'right': ['right', 'left', 'bottom', 'top', 'center'],
    'center': ['center']
  }
  
  // 尝试找到最佳位置
  let finalPosition = preferredPosition
  let top = 0
  let left = 0
  
  for (const pos of positionPriority[preferredPosition as keyof typeof positionPriority] || ['center']) {
    if (pos === 'center') {
      top = (windowHeight - cardHeight) / 2 + scrollTop
      left = (windowWidth - cardWidth) / 2 + scrollLeft
      finalPosition = 'center'
      break
    }
    
    // 检查是否有足够空间
    if (spaces[pos as keyof typeof spaces] >= 0) {
      switch (pos) {
        case 'top':
          top = targetRect.top - cardHeight - gap + scrollTop
          left = targetRect.left + (targetRect.width - cardWidth) / 2 + scrollLeft
          break
        case 'bottom':
          top = targetRect.bottom + gap + scrollTop
          left = targetRect.left + (targetRect.width - cardWidth) / 2 + scrollLeft
          break
        case 'left':
          top = targetRect.top + (targetRect.height - cardHeight) / 2 + scrollTop
          left = targetRect.left - cardWidth - gap + scrollLeft
          break
        case 'right':
          top = targetRect.top + (targetRect.height - cardHeight) / 2 + scrollTop
          left = targetRect.right + gap + scrollLeft
          break
      }
      
      // 微调确保在边界内
      left = Math.max(gap + scrollLeft, Math.min(left, windowWidth - cardWidth - gap + scrollLeft))
      top = Math.max(gap + scrollTop, Math.min(top, windowHeight - cardHeight - gap + scrollTop))
      
      finalPosition = pos
      break
    }
  }
  
  // 更新实际位置状态
  actualCardPosition.value = finalPosition
  
  return { top, left, position: finalPosition }
}

// 获取箭头方向（基于实际位置）
const getArrowDirection = () => {
  const position = actualCardPosition.value
  switch (position) {
    case 'top': return 'down'
    case 'bottom': return 'up'
    case 'left': return 'right'
    case 'right': return 'left'
    default: return 'none'
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

// 监听窗口大小变化和滚动
const handleResize = () => {
  // 延迟更新，避免频繁计算
  setTimeout(() => {
    updateHighlight()
  }, 100)
}

const handleScroll = () => {
  updateHighlight()
}

// 组件挂载时检查是否需要显示引导
onMounted(() => {
  checkShouldShowTutorial()
  window.addEventListener('resize', handleResize)
  window.addEventListener('scroll', handleScroll, { passive: true })
})

// 组件销毁时清理事件监听器
onBeforeUnmount(() => {
  window.removeEventListener('resize', handleResize)
  window.removeEventListener('scroll', handleScroll)
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
  background: rgba(0, 0, 0, 0.75);
  backdrop-filter: blur(8px) saturate(120%);
  animation: backdropFadeIn 0.5s cubic-bezier(0.4, 0, 0.2, 1);
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
  border: 3px solid var(--header-bg, #2c7a7b);
  border-radius: 16px;
  box-shadow: 
    0 0 0 4px rgba(44, 122, 123, 0.4),
    0 0 0 8px rgba(44, 122, 123, 0.2),
    0 0 0 9999px rgba(0, 0, 0, 0.5),
    inset 0 0 0 2px rgba(255, 255, 255, 0.3);
  transition: all 0.5s cubic-bezier(0.4, 0, 0.2, 1);
  animation: highlightPulse 2.5s infinite;
}

@keyframes highlightPulse {
  0%, 100% {
    box-shadow: 
      0 0 0 4px rgba(44, 122, 123, 0.4),
      0 0 0 8px rgba(44, 122, 123, 0.2),
      0 0 0 9999px rgba(0, 0, 0, 0.5),
      inset 0 0 0 2px rgba(255, 255, 255, 0.3);
    transform: scale(1);
  }
  50% {
    box-shadow: 
      0 0 0 8px rgba(44, 122, 123, 0.6),
      0 0 0 16px rgba(44, 122, 123, 0.3),
      0 0 0 9999px rgba(0, 0, 0, 0.5),
      inset 0 0 0 2px rgba(255, 255, 255, 0.5);
    transform: scale(1.02);
  }
}

.tutorial-card {
  position: absolute;
  background: linear-gradient(135deg, rgba(255, 255, 255, 0.95) 0%, rgba(255, 255, 255, 0.9) 100%);
  backdrop-filter: blur(20px) saturate(150%);
  border-radius: 20px;
  border: 1px solid rgba(255, 255, 255, 0.3);
  box-shadow: 
    0 32px 64px rgba(0, 0, 0, 0.25),
    0 16px 32px rgba(44, 122, 123, 0.15),
    inset 0 1px 0 rgba(255, 255, 255, 0.4);
  min-width: 380px;
  max-width: 420px;
  overflow: hidden;
  animation: cardSlideIn 0.6s cubic-bezier(0.2, 0, 0.1, 1);
  z-index: 3;
  transform-origin: center;
}

@keyframes cardSlideIn {
  from {
    opacity: 0;
    transform: scale(0.85) translateY(40px) rotateX(15deg);
    filter: blur(8px);
  }
  to {
    opacity: 1;
    transform: scale(1) translateY(0) rotateX(0deg);
    filter: blur(0px);
  }
}

@keyframes backdropFadeIn {
  from { 
    opacity: 0;
    backdrop-filter: blur(0px) saturate(100%);
  }
  to { 
    opacity: 1;
    backdrop-filter: blur(8px) saturate(120%);
  }
}

/* 箭头指示器 */
.tutorial-arrow {
  position: absolute;
  width: 0;
  height: 0;
  z-index: 4;
  animation: arrowFloat 2s ease-in-out infinite;
}

@keyframes arrowFloat {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(-3px); }
}

.arrow-up {
  bottom: -15px;
  left: 50%;
  transform: translateX(-50%);
  border-left: 15px solid transparent;
  border-right: 15px solid transparent;
  border-top: 15px solid rgba(255, 255, 255, 0.95);
  filter: drop-shadow(0 4px 12px rgba(0, 0, 0, 0.15));
}

.arrow-down {
  top: -15px;
  left: 50%;
  transform: translateX(-50%);
  border-left: 15px solid transparent;
  border-right: 15px solid transparent;
  border-bottom: 15px solid rgba(255, 255, 255, 0.95);
  filter: drop-shadow(0 -4px 12px rgba(0, 0, 0, 0.15));
}

.arrow-left {
  right: -15px;
  top: 50%;
  transform: translateY(-50%);
  border-top: 15px solid transparent;
  border-bottom: 15px solid transparent;
  border-left: 15px solid rgba(255, 255, 255, 0.95);
  filter: drop-shadow(4px 0 12px rgba(0, 0, 0, 0.15));
}

.arrow-right {
  left: -15px;
  top: 50%;
  transform: translateY(-50%);
  border-top: 15px solid transparent;
  border-bottom: 15px solid transparent;
  border-right: 15px solid rgba(255, 255, 255, 0.95);
  filter: drop-shadow(-4px 0 12px rgba(0, 0, 0, 0.15));
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
  padding: 24px 28px 20px;
  background: linear-gradient(135deg, var(--header-bg, #2c7a7b) 0%, #319795 50%, #2dd4bf 100%);
  color: white;
  position: relative;
  overflow: hidden;
}

.tutorial-header::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: linear-gradient(45deg, rgba(255, 255, 255, 0.1) 0%, transparent 100%);
  pointer-events: none;
}

.tutorial-step-indicator {
  display: flex;
  align-items: center;
  background: rgba(255, 255, 255, 0.25);
  border-radius: 20px;
  padding: 6px 12px;
  font-size: 12px;
  font-weight: 700;
  border: 1px solid rgba(255, 255, 255, 0.2);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  backdrop-filter: blur(8px);
  position: relative;
  z-index: 1;
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
  font-size: 20px;
  font-weight: 700;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
  position: relative;
  z-index: 1;
}

.tutorial-close {
  background: rgba(255, 255, 255, 0.25);
  border: 1px solid rgba(255, 255, 255, 0.3);
  border-radius: 50%;
  width: 36px;
  height: 36px;
  color: white;
  cursor: pointer;
  font-size: 18px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  backdrop-filter: blur(8px);
  position: relative;
  z-index: 1;
}

.tutorial-close:hover {
  background: rgba(255, 255, 255, 0.35);
  transform: scale(1.1) rotate(90deg);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
}

.tutorial-content {
  padding: 28px;
  background: rgba(255, 255, 255, 0.02);
}

.tutorial-description {
  margin: 0 0 24px 0;
  line-height: 1.7;
  color: #2d3748;
  font-size: 16px;
  font-weight: 500;
  text-shadow: 0 1px 2px rgba(255, 255, 255, 0.5);
}

.demo-section {
  margin: 20px 0;
  text-align: center;
}

.shortcut-demo {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 24px;
  background: linear-gradient(135deg, #e0f2f1 0%, #b2dfdb 100%);
  border-radius: 16px;
  border: 2px solid var(--header-bg, #2c7a7b);
  position: relative;
  overflow: hidden;
  box-shadow: 0 8px 16px rgba(44, 122, 123, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.3);
}

.shortcut-demo::before {
  content: '';
  position: absolute;
  top: 0;
  left: -100%;
  width: 100%;
  height: 100%;
  background: linear-gradient(90deg, transparent, rgba(44, 122, 123, 0.2), transparent);
  animation: shimmer 2.5s infinite;
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
  font-size: 20px;
  font-weight: bold;
  color: var(--header-bg, #2c7a7b);
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.1);
}

.tray-demo {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  padding: 24px;
  background: linear-gradient(135deg, #e0f2f1 0%, #b2dfdb 100%);
  border-radius: 16px;
  border: 2px solid var(--header-bg, #2c7a7b);
  box-shadow: 0 8px 16px rgba(44, 122, 123, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.3);
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
  border: 3px solid var(--header-bg, #2c7a7b);
  border-radius: 50%;
  animation: clickPulse 1.8s infinite;
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
  gap: 16px;
  padding: 24px;
  background: linear-gradient(135deg, #e0f2f1 0%, #b2dfdb 100%);
  border-radius: 16px;
  border: 2px solid var(--header-bg, #2c7a7b);
  box-shadow: 0 8px 16px rgba(44, 122, 123, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.3);
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
  background: var(--header-bg, #2c7a7b);
  color: white;
  padding: 6px 12px;
  border-radius: 16px;
  font-size: 13px;
  font-weight: 700;
  box-shadow: 0 2px 8px rgba(44, 122, 123, 0.3);
  animation: bounce 1.2s infinite;
}

@keyframes bounce {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(-4px); }
}

.arrow-down {
  font-size: 28px;
  color: var(--header-bg, #2c7a7b);
  text-shadow: 0 2px 4px rgba(44, 122, 123, 0.3);
  animation: moveDown 1.2s infinite;
}

@keyframes moveDown {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(4px); }
}

.result {
  padding: 10px 20px;
  background: linear-gradient(135deg, #319795, #2dd4bf);
  color: white;
  border-radius: 12px;
  font-weight: 700;
  font-size: 15px;
  box-shadow: 0 4px 12px rgba(45, 212, 191, 0.3);
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);
}

.demo-tip {
  margin: 8px 0 0 0;
  font-size: 13px;
  color: #666;
  font-style: italic;
}

.tutorial-footer {
  padding: 20px 28px 28px;
  background: linear-gradient(135deg, rgba(248, 249, 250, 0.95) 0%, rgba(241, 243, 244, 0.9) 100%);
  border-top: 1px solid rgba(0, 0, 0, 0.06);
  backdrop-filter: blur(8px);
}

.tutorial-progress {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 20px;
}

.progress-bar {
  flex: 1;
  height: 8px;
  background: linear-gradient(135deg, #e2e8f0 0%, #cbd5e0 100%);
  border-radius: 6px;
  overflow: hidden;
  box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.1);
  border: 1px solid rgba(44, 122, 123, 0.1);
}

.progress-fill {
  height: 100%;
  background: linear-gradient(90deg, var(--header-bg, #2c7a7b) 0%, #319795 50%, #2dd4bf 100%);
  transition: width 0.6s cubic-bezier(0.4, 0, 0.2, 1);
  border-radius: 4px;
  box-shadow: 0 2px 4px rgba(44, 122, 123, 0.3), inset 0 1px 0 rgba(255, 255, 255, 0.2);
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
  padding: 12px 24px;
  border: none;
  border-radius: 12px;
  font-size: 15px;
  font-weight: 700;
  cursor: pointer;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  text-decoration: none;
  display: flex;
  align-items: center;
  gap: 6px;
  position: relative;
  overflow: hidden;
}

.btn-primary {
  background: linear-gradient(135deg, var(--header-bg, #2c7a7b) 0%, #319795 50%, #2dd4bf 100%);
  color: white;
  box-shadow: 0 4px 12px rgba(44, 122, 123, 0.3), inset 0 1px 0 rgba(255, 255, 255, 0.2);
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);
  border: 1px solid rgba(255, 255, 255, 0.2);
}

.btn-primary:hover {
  transform: translateY(-2px) scale(1.02);
  box-shadow: 0 8px 20px rgba(44, 122, 123, 0.4), inset 0 1px 0 rgba(255, 255, 255, 0.3);
  background: linear-gradient(135deg, #319795 0%, #2dd4bf 50%, #4ade80 100%);
}

.btn-complete {
  background: linear-gradient(135deg, #f59e0b 0%, #eab308 50%, #84cc16 100%);
  box-shadow: 0 4px 12px rgba(245, 158, 11, 0.3), inset 0 1px 0 rgba(255, 255, 255, 0.2);
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);
  border: 1px solid rgba(255, 255, 255, 0.2);
}

.btn-secondary {
  background: linear-gradient(135deg, rgba(248, 249, 250, 0.9) 0%, rgba(241, 243, 244, 0.9) 100%);
  color: #4a5568;
  border: 1px solid rgba(44, 122, 123, 0.2);
  backdrop-filter: blur(8px);
  box-shadow: 0 2px 8px rgba(44, 122, 123, 0.1);
}

.btn-secondary:hover {
  background: linear-gradient(135deg, rgba(226, 232, 240, 0.9) 0%, rgba(203, 213, 224, 0.9) 100%);
  transform: translateY(-2px) scale(1.02);
  box-shadow: 0 4px 12px rgba(44, 122, 123, 0.2);
  border-color: rgba(44, 122, 123, 0.3);
}

.btn-link {
  background: rgba(44, 122, 123, 0.05);
  color: #4a5568;
  padding: 12px 20px;
  font-size: 14px;
  border-radius: 12px;
  font-weight: 600;
  backdrop-filter: blur(4px);
}

.btn-link:hover {
  color: var(--header-bg, #2c7a7b);
  background: rgba(44, 122, 123, 0.1);
  transform: translateY(-1px);
  box-shadow: 0 2px 8px rgba(44, 122, 123, 0.15);
}

/* 响应式设计优化 */
@media (max-width: 768px) {
  .tutorial-card {
    min-width: 340px;
    max-width: calc(100vw - 32px);
    margin: 0 16px;
  }
  
  .tutorial-header {
    padding: 20px 20px 16px;
    flex-wrap: wrap;
    gap: 8px;
  }
  
  .tutorial-title {
    font-size: 18px;
    flex: 1;
    min-width: 200px;
  }
  
  .tutorial-content {
    padding: 20px;
  }
  
  .tutorial-description {
    font-size: 15px;
    line-height: 1.6;
    margin-bottom: 20px;
  }
  
  .tutorial-footer {
    padding: 16px 20px 20px;
  }
  
  .tutorial-actions {
    flex-wrap: wrap;
    gap: 8px;
    justify-content: center;
  }
  
  .btn {
    padding: 10px 18px;
    font-size: 14px;
    min-width: 100px;
  }
  
  .btn-link {
    padding: 8px 16px;
    font-size: 13px;
  }
}

@media (max-width: 480px) {
  .tutorial-card {
    min-width: 300px;
    max-width: calc(100vw - 24px);
    margin: 0 12px;
  }
  
  .tutorial-header {
    padding: 16px 16px 12px;
  }
  
  .tutorial-title {
    font-size: 16px;
  }
  
  .tutorial-close {
    width: 32px;
    height: 32px;
    font-size: 16px;
  }
  
  .tutorial-content {
    padding: 16px;
  }
  
  .tutorial-description {
    font-size: 14px;
    margin-bottom: 16px;
  }
  
  .shortcut-demo,
  .tray-demo,
  .action-demo {
    padding: 16px;
    border-radius: 12px;
  }
  
  .demo-section {
    margin: 16px 0;
  }
  
  .tutorial-footer {
    padding: 12px 16px 16px;
  }
  
  .tutorial-progress {
    margin-bottom: 16px;
  }
  
  .tutorial-actions {
    flex-direction: column;
    align-items: stretch;
  }
  
  .btn {
    padding: 12px 20px;
    font-size: 15px;
    justify-content: center;
    width: 100%;
  }
  
  .btn-link {
    order: -1;
    padding: 8px 16px;
    font-size: 13px;
  }
}

@media (max-height: 600px) {
  .tutorial-card {
    max-height: calc(100vh - 40px);
    overflow-y: auto;
  }
  
  .tutorial-content {
    max-height: 300px;
    overflow-y: auto;
  }
}

/* 自动粘贴警告样式 */
.auto-paste-warning {
  display: flex;
  gap: 16px;
  padding: 20px;
  background: linear-gradient(135deg, #fef3c7 0%, #fed7aa 100%);
  border: 2px solid #f59e0b;
  border-radius: 16px;
  animation: warningPulse 2.5s infinite;
  box-shadow: 0 8px 16px rgba(245, 158, 11, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.4);
}

@keyframes warningPulse {
  0%, 100% {
    box-shadow: 0 8px 16px rgba(245, 158, 11, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.4), 0 0 0 0 rgba(245, 158, 11, 0.4);
  }
  50% {
    box-shadow: 0 8px 16px rgba(245, 158, 11, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.4), 0 0 0 8px rgba(245, 158, 11, 0.2);
  }
}

.auto-paste-warning .warning-icon {
  font-size: 20px;
  line-height: 1;
  margin-top: 2px;
}

.auto-paste-warning .warning-content {
  flex: 1;
  min-width: 0;
}

.auto-paste-warning .warning-title {
  font-size: 14px;
  font-weight: 700;
  color: #856404;
  margin-bottom: 6px;
}

.auto-paste-warning .warning-text {
  font-size: 13px;
  line-height: 1.4;
  color: #6c5429;
}
</style> 