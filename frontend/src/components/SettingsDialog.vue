<template>
  <div v-if="modelValue" class="settings-overlay" @click.self="handleClose">
    <div class="settings-dialog responsive-dialog" :class="responsiveClasses">
      <div class="settings-header">
        <h2 class="settings-title">设置</h2>
        <button class="close-button" @click="handleClose">×</button>
      </div>

      <div class="settings-content">
        <div class="settings-group">
          <div class="settings-item">
            <div class="settings-label">
              <span>开机自启</span>
              <span class="settings-description">启动系统时自动运行应用</span>
            </div>
            <label class="switch">
              <input type="checkbox" :checked="settings.auto_start === 1" @change="(e: Event) => settings.auto_start = (e.target as HTMLInputElement).checked ? 1 : 0">
              <span class="slider"></span>
            </label>
          </div>

          <div class="settings-item">
            <div class="settings-label">
              <span>最大记录条数</span>
              <span class="settings-description">超过此数量将自动清理旧记录</span>
            </div>
            <div class="number-input">
              <button class="number-button" @click="decreaseMaxRecords">-</button>
              <input type="number" v-model.number="settings.max_records" min="50" max="1000" autocomplete="off">
              <button class="number-button" @click="increaseMaxRecords">+</button>
            </div>
          </div>

          <div class="settings-item">
            <div class="settings-label">
              <span>窗口快捷键</span>
              <span class="settings-description">按下快捷键显示/隐藏窗口</span>
            </div>
            <div class="shortcut-input" :class="{ 'recording': isRecording, 'error': shortcutError }" @click.stop="startRecording">
              <template v-if="isRecording">
                <span class="recording-text">
                  {{ pressedKeys.length > 0 ? pressedKeys.join('+') : '请按下快捷键组合...' }}
                </span>
              </template>
                              <template v-else>
                  <span>{{ displayShortcut || '点击设置' }}</span>
                  <span v-if="shortcutError" class="error-icon">⚠️</span>
                </template>
            </div>
            <div v-if="shortcutError" class="error-message">{{ shortcutError }}</div>
          </div>

          <div class="settings-item">
            <div class="settings-label">
              <span>云同步</span>
              <span class="settings-description">同步剪贴板内容到云端</span>
            </div>
            <label class="switch">
              <input type="checkbox" :checked="settings.cloud_sync === 1" @change="handleCloudSyncChange">
              <span class="slider"></span>
            </label>
          </div>

          <!-- VIP状态显示 -->
          <div class="settings-item vip-info-item">
            <div class="settings-label">
              <span>会员状态</span>
              <span class="settings-description">查看当前会员权益和使用情况</span>
            </div>
            <div class="vip-status-display">
              <div class="vip-status" :class="{ 'is-vip': vipStore.isVip }">
                <span class="vip-icon">{{ vipStore.isVip ? '👑' : '🆓' }}</span>
                <span class="vip-type">{{ vipStore.vipTypeDisplay }}</span>
                <span v-if="vipStore.isVip && vipStore.remainingDays.value > 0" class="vip-remaining">
                  (剩余{{ vipStore.remainingDays.value }}天)
                </span>
              </div>
              <button class="upgrade-button" @click="showVipDialog = true">
                {{ vipStore.isVip ? '管理会员' : '升级VIP' }}
              </button>
            </div>
          </div>

          <!-- VIP功能限制显示 -->
          <div class="settings-item vip-limits-item">
            <div class="settings-label">
              <span>功能限制</span>
              <span class="settings-description">当前账户可用功能和限制</span>
            </div>
            <div class="vip-limits-display">
              <div class="limit-item">
                <span class="limit-label">本地记录:</span>
                <span class="limit-value">{{ vipStore.maxRecordsLimit }}条</span>
              </div>
              <div class="limit-item">
                <span class="limit-label">云同步:</span>
                <span class="limit-value" :class="{ 'vip-feature': vipStore.canCloudSync }">
                  {{ vipStore.canCloudSync ? (vipStore.isVip ? '完整支持' : '10条体验') : '不支持' }}
                </span>
              </div>
              <div class="limit-item">
                <span class="limit-label">文件上传:</span>
                <span class="limit-value" :class="{ 'vip-feature': vipStore.isVip }">
                  {{ vipStore.isVip ? '支持5MB以下' : '不支持' }}
                </span>
              </div>
            </div>
          </div>

          <div class="settings-item-wrapper auto-paste-setting">
            <div class="settings-item">
              <div class="settings-label">
                <span>自动粘贴</span>
                <span class="settings-description">双击卡片后自动粘贴到之前获得焦点的窗口</span>
              </div>
              <label class="switch">
                <input type="checkbox" :checked="settings.auto_paste === 1" @change="(e: Event) => settings.auto_paste = (e.target as HTMLInputElement).checked ? 1 : 0">
                <span class="slider"></span>
              </label>
            </div>
            
            <div v-if="settings.auto_paste === 1" class="settings-warning">
              <div class="warning-icon">⚠️</div>
              <div class="warning-content">
                <div class="warning-title">使用注意</div>
                <div class="warning-text">
                  某些应用可能自定义了Ctrl+V快捷键，可根据实际使用情况选择是否开启自动粘贴。
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div class="settings-footer">
        <button class="cancel-button" @click="handleClose">取消</button>
        <button class="confirm-button" @click="handleConfirm" :disabled="isSaving || hasErrors">
          {{ isSaving ? '保存中...' : '确认' }}
        </button>
      </div>
    </div>
    
    <!-- VIP升级对话框 -->
    <VipUpgradeDialog v-model="showVipDialog" />
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onBeforeUnmount, computed, inject } from 'vue';
import { listen } from '@tauri-apps/api/event';
import { useWindowAdaptive, generateResponsiveClasses } from '../utils/responsive';
import { settingsApi, isSuccess } from '../utils/api';
import { useUserStore } from '../utils/userStore';
import { useVipStore } from '../utils/vipStore';
import VipUpgradeDialog from './VipUpgradeDialog.vue';

const props = defineProps<{
  modelValue: boolean
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'save', settings: Settings): void
}>();

interface Settings {
  auto_start: number;  // 0 关闭 1 开启
  max_records: number;
  shortcut_key: string;
  cloud_sync: number;  // 0 关闭 1 开启
  auto_paste: number;  // 0 关闭 1 开启
}

const settings = ref<Settings>({
  auto_start: 0,
  max_records: 200,
  shortcut_key: 'Ctrl+`',
  cloud_sync: 0,
  auto_paste: 1
});

const isRecording = ref(false);
const isSaving = ref(false);
const shortcutError = ref('');
const pressedKeys = ref<string[]>([]);

const showMessageBar = inject('showMessageBar') as (message: string, type?: 'info' | 'warning' | 'error') => void;
let cloudSyncDisabledListener: (() => void) | null = null;

// 用户状态管理
const userStore = useUserStore();

// VIP状态管理
const vipStore = useVipStore();
const showVipDialog = ref(false);

// 使用响应式工具
const responsive = useWindowAdaptive();
const responsiveClasses = computed(() => generateResponsiveClasses(responsive));

// 使用统一的平台检测
const isMac = computed(() => responsive.isMac.value);

// 计算是否有错误
const hasErrors = computed(() => {
  return shortcutError.value !== '' || 
         settings.value.max_records < 50 || 
         settings.value.max_records > 1000;
});

// 快捷键显示适配
const displayShortcut = computed(() => {
  if (!settings.value.shortcut_key) return '';
  
  let displayKey = settings.value.shortcut_key;
  if (isMac) {
    // Mac上显示适配：Meta -> Cmd（因为Meta对应Mac的Cmd键）
    displayKey = displayKey.replace(/\bMeta\b/g, 'Cmd');
  }
  return displayKey;
});

// 将显示格式转换为存储格式
const convertDisplayToStorage = (displayKey: string): string => {
  if (!isMac) return displayKey;
  
  // Mac上存储适配：Cmd -> Meta（因为Mac的Cmd键对应后端的Meta）
  return displayKey.replace(/\bCmd\b/g, 'Meta');
};

// 处理云同步开关切换
const handleCloudSyncChange = (e: Event) => {
  const target = e.target as HTMLInputElement;
  const isEnabled = target.checked;
  
  if (isEnabled) {
    // 用户尝试开启云同步，检查是否已登录
    if (!userStore.isLoggedIn()) {
      // 用户未登录，阻止开关切换并显示提示
      target.checked = false;
      settings.value.cloud_sync = 0;
      
      if (showMessageBar) {
        showMessageBar('请先登录账号才能开启云同步功能', 'warning');
      } else {
        alert('请先登录账号才能开启云同步功能');
      }
      return;
    }
  }
  
  // 更新设置值
  settings.value.cloud_sync = isEnabled ? 1 : 0;
};

// 加载设置的统一函数
const loadSettings = async () => {
  try {
    const response = await settingsApi.loadSettings();
    if (!isSuccess(response)) return;
    const currentSettings = response.data;
    console.log('当前设置:', currentSettings);
    settings.value = { ...currentSettings };
    // 清除错误状态
    shortcutError.value = '';
  } catch (error) {
    console.error('加载设置失败:', error);
  }
};



// 监听弹窗打开时加载设置
watch(() => props.modelValue, async (newVal) => {
  if (newVal) {
    await loadSettings();
  }
});



const handleClose = () => {
  emit('update:modelValue', false);
};

const handleConfirm = async () => {
  if (hasErrors.value) {
    return;
  }
  
  isSaving.value = true;
  try {
    const response = await settingsApi.saveSettings({ settings: settings.value });
    if (!isSuccess(response)) {
      console.error('设置保存失败');
      return;
    }
    emit('save', settings.value);
    handleClose();
  } catch (error) {
    console.error('保存设置失败:', error);
    // 显示错误信息给用户
    alert(`保存设置失败: ${error}`);
  } finally {
    isSaving.value = false;
  }
};

const decreaseMaxRecords = () => {
  if (settings.value.max_records > 50) {
    settings.value.max_records -= 50;
  }
};

const increaseMaxRecords = () => {
  if (settings.value.max_records < 1000) {
    settings.value.max_records += 50;
  }
};

// 开始录制时清空数组，确保每次独立记录
const startRecording = (_e: any) => {
  isRecording.value = true;
  pressedKeys.value = [];
  shortcutError.value = '';
};

// 停止录制时清空数组，避免残留
const stopRecording = () => {
  isRecording.value = false;
  pressedKeys.value = [];
};

// 验证快捷键
const validateShortcut = async (shortcut: string) => {
  try {
    // 验证时需要转换为存储格式
    const storageFormat = convertDisplayToStorage(shortcut);
    const response = await settingsApi.validateShortcut(storageFormat);
    const isValid = isSuccess(response) && response.data;
    if (!isValid) {
      shortcutError.value = '快捷键不可用或已被占用';
    } else {
      shortcutError.value = '';
    }
    return isValid;
  } catch (error) {
    shortcutError.value = '快捷键验证失败';
    return false;
  }
};



// 精准记录所有按下的键（含修饰键+普通键+特殊键）
const handleKeyDown = async (e: KeyboardEvent) => {
  if (!isRecording.value) return;
  e.preventDefault();

  // 1. 识别当前按下的修饰键（Ctrl/Shift/Alt/Meta）
  const modifiers = [];
  if (e.ctrlKey) modifiers.push('Ctrl');
  if (e.shiftKey) modifiers.push('Shift');
  if (e.altKey) modifiers.push('Alt');
  if (e.metaKey) modifiers.push(isMac ? 'Cmd' : 'Meta');

  // 2. 处理普通键（映射特殊键，保证可读性）
  let key = e.key;
  
  // 过滤修饰键本身，避免重复添加（如Ctrl+Control）
  const modifierKeyNames = ['Control', 'Shift', 'Alt', 'Meta'];
  const isModifierKey = modifierKeyNames.includes(key);
  
  // 如果是修饰键本身，只更新显示但不添加到普通键
  if (!isModifierKey) {
    const keyMap: { [k: string]: string } = {
      ' ': 'Space',
      'Escape': 'Escape',
      // 保持箭头键原始名称，与后端一致
      'ArrowUp': 'ArrowUp',
      'ArrowDown': 'ArrowDown',
      'ArrowLeft': 'ArrowLeft',
      'ArrowRight': 'ArrowRight',
      'Backspace': 'Backspace',
      'Delete': 'Delete',
      'Enter': 'Enter',
      'Tab': 'Tab',
      'Insert': 'Insert',
      'Home': 'Home',
      'End': 'End',
      'PageUp': 'PageUp',
      'PageDown': 'PageDown',
      // Backquote键映射
      'Backquote': '`',
      '`': '`'
    };
    key = keyMap[key] || key; // 特殊键映射
    if (key.length === 1) key = key.toUpperCase(); // 单个字符转大写
  }

  // 3. 更新pressedKeys数组 - 始终显示当前状态
  pressedKeys.value = [...modifiers];
  if (!isModifierKey) {
    pressedKeys.value.push(key);
  }

  // 4. 限制最大按键数（最多4个，避免无意义组合）
  if (pressedKeys.value.length > 4) {
    pressedKeys.value = pressedKeys.value.slice(-4); // 保留最后4个
  }

  // 5. 保存条件：至少1个修饰键 + 1个普通键
  const hasModifier = modifiers.length > 0;
  const regularKeys = ['Ctrl', 'Shift', 'Alt', 'Meta', 'Cmd'];
  const hasRegularKey = !isModifierKey && !regularKeys.includes(key);
  
  if (hasModifier && hasRegularKey) {
    const newShortcut = pressedKeys.value.join('+'); // 按顺序拼接
    const currentShortcut = displayShortcut.value;
    
    // 如果新快捷键和当前设置一样，直接保存
    if (newShortcut === currentShortcut) {
      stopRecording();
      return;
    }
    
    // 实时验证快捷键
    const isValid = await validateShortcut(newShortcut);
    if (isValid) {
      // 保存时转换为存储格式
      settings.value.shortcut_key = convertDisplayToStorage(newShortcut);
      stopRecording(); // 录制完成
    }
  }
};

// 键释放时精准移除（含修饰键状态检查）
const handleKeyUp = (e: KeyboardEvent) => {
  if (!isRecording.value) return;

  // 1. 重新计算当前状态的修饰键（基于事件状态而非释放的键）
  const currentModifiers = [];
  if (e.ctrlKey) currentModifiers.push('Ctrl');
  if (e.shiftKey) currentModifiers.push('Shift');
  if (e.altKey) currentModifiers.push('Alt');
  if (e.metaKey) currentModifiers.push(isMac ? 'Cmd' : 'Meta');

  // 2. 处理释放的键
  let key = e.key;
  const modifierKeyNames = ['Control', 'Shift', 'Alt', 'Meta'];
  const isModifierKey = modifierKeyNames.includes(key);
  
  if (!isModifierKey) {
    const keyMap: { [k: string]: string } = {
      ' ': 'Space',
      'Escape': 'Escape',
      // 保持箭头键原始名称，与后端一致
      'ArrowUp': 'ArrowUp',
      'ArrowDown': 'ArrowDown',
      'ArrowLeft': 'ArrowLeft',
      'ArrowRight': 'ArrowRight',
      'Backspace': 'Backspace',
      'Delete': 'Delete',
      'Enter': 'Enter',
      'Tab': 'Tab',
      'Insert': 'Insert',
      'Home': 'Home',
      'End': 'End',
      'PageUp': 'PageUp',
      'PageDown': 'PageDown',
      // Backquote键映射
      'Backquote': '`',
      '`': '`'
    };
    key = keyMap[key] || key;
    if (key.length === 1) key = key.toUpperCase();

    // 从数组中移除释放的普通键
    const index = pressedKeys.value.indexOf(key);
    if (index > -1) {
      pressedKeys.value.splice(index, 1);
    }
  }

  // 3. 更新pressedKeys数组以反映当前修饰键状态
  // 移除所有修饰键，然后添加当前按下的修饰键
  const regularKeys = ['Ctrl', 'Shift', 'Alt', 'Meta', 'Cmd'];
  pressedKeys.value = pressedKeys.value.filter(k => !regularKeys.includes(k));
  
  // 添加当前仍按下的修饰键到开头
  pressedKeys.value = [...currentModifiers, ...pressedKeys.value];
};

// 点击外部时停止录制（原有逻辑保留）
const handleClickOutside = (e: MouseEvent) => {
  if (isRecording.value) {
    const target = e.target as HTMLElement;
    if (!target.closest('.shortcut-input')) {
      stopRecording();
    }
  }
};

onMounted(async () => {
  window.addEventListener('keydown', handleKeyDown);
  window.addEventListener('keyup', handleKeyUp);
  window.addEventListener('click', handleClickOutside);

  // 监听云同步禁用事件
  cloudSyncDisabledListener = await listen('cloud-sync-disabled', async () => {
    console.log('设置页面收到云同步禁用事件，重新加载设置');
    // 重新加载设置以反映云同步已被禁用
    await loadSettings();
    // 如果设置页面当前可见，显示提示信息
    if (props.modelValue && showMessageBar) {
      showMessageBar('云同步已被关闭', 'info');
    }
  });
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeyDown);
  window.removeEventListener('keyup', handleKeyUp);
  window.removeEventListener('click', handleClickOutside);
  
  // 清理云同步事件监听器
  if (cloudSyncDisabledListener) {
    cloudSyncDisabledListener();
  }
});
</script>

<style scoped>
.settings-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 1000;
  backdrop-filter: blur(4px);
  padding: var(--spacing-xl);
  box-sizing: border-box;
  overflow: hidden;
}

.settings-dialog {
  background: var(--card-bg);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-xl);
  display: flex;
  flex-direction: column;
  animation: dialog-in 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  overflow: hidden;
  margin: 0 auto;
  /* 设置弹窗专用的字体放大 */
  --settings-font-scale: 1.25; /* 适度放大25% */
  
  /* 优化字体渲染 */
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC', 'Hiragino Sans GB', 'Microsoft YaHei', 'Helvetica Neue', Helvetica, Arial, sans-serif;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  text-rendering: optimizeLegibility;
  font-variant-ligatures: normal;
  
  /* 优化文字颜色对比度 */
  --settings-text-primary: #1a1a1a;
  --settings-text-secondary: #666666;
}

/* 响应式弹窗已在responsive.css中定义，这里扩展设置弹窗特定样式 */

@keyframes dialog-in {
  from {
    opacity: 0;
    transform: scale(0.95);
  }

  to {
    opacity: 1;
    transform: scale(1);
  }
}

.settings-header {
  padding: var(--spacing-lg) var(--spacing-xl);
  border-bottom: var(--border-width) solid var(--border-color);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.settings-title {
  margin: 0;
  font-size: calc(var(--text-xl) * var(--settings-font-scale) * 1.2);
  color: var(--settings-text-primary);
  font-weight: 700;
  letter-spacing: 0.5px;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
  font-feature-settings: 'kern' 1;
}

.close-button {
  background: none;
  border: none;
  font-size: calc(24px * var(--settings-font-scale));
  color: var(--text-secondary, #666);
  cursor: pointer;
  padding: calc(4px + (var(--settings-font-scale) - 1) * 1px);
  line-height: 1;
  border-radius: calc(4px + (var(--settings-font-scale) - 1) * 1px);
  transition: all 0.2s ease;
}

.close-button:hover {
  background: var(--hover-bg, rgba(0, 0, 0, 0.05));
  color: var(--text-primary, #2d3748);
}

.settings-content {
  padding: var(--spacing-xl) var(--spacing-2xl);
  overflow-y: auto;
  overflow-x: hidden;
  flex: 1;
  min-height: 0;
  max-height: calc(90vh - 7.5rem);
}

.settings-group {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-lg);
}

.settings-item-wrapper {
  display: flex;
  flex-direction: column;
}

.settings-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--spacing-xl);
}

.settings-label {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-xs);
  flex: 1;
  min-width: 0;
  overflow: hidden;
}

.settings-label span:first-child {
  font-weight: 600;
  color: var(--settings-text-primary);
  font-size: calc(var(--text-lg) * var(--settings-font-scale) * 1.1);
  letter-spacing: 0.3px;
  text-shadow: 0 0.5px 1px rgba(0, 0, 0, 0.04);
  font-feature-settings: 'kern' 1;
}

.settings-description {
  font-size: calc(var(--text-sm) * var(--settings-font-scale) * 1);
  color: var(--settings-text-secondary);
  line-height: 1.4;
  font-weight: 400;
  opacity: 0.9;
  font-feature-settings: 'kern' 1;
}

/* 开关样式 */
.switch {
  position: relative;
  display: inline-block;
  width: calc(44px + (var(--settings-font-scale) - 1) * 8px);
  height: calc(24px + (var(--settings-font-scale) - 1) * 4px);
}

.switch input {
  opacity: 0;
  width: 0;
  height: 0;
}

.slider {
  position: absolute;
  cursor: pointer;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: var(--switch-bg, #ccc);
  transition: .3s;
  border-radius: 24px;
}

.slider:before {
  position: absolute;
  content: "";
  height: calc(20px + (var(--settings-font-scale) - 1) * 3px);
  width: calc(20px + (var(--settings-font-scale) - 1) * 3px);
  left: calc(2px + (var(--settings-font-scale) - 1) * 1px);
  bottom: calc(2px + (var(--settings-font-scale) - 1) * 0.5px);
  background-color: white;
  transition: .3s;
  border-radius: 50%;
}

input:checked+.slider {
  background-color: var(--primary-color, #2c7a7b);
}

input:checked+.slider:before {
  transform: translateX(calc(20px + (var(--settings-font-scale) - 1) * 4px));
}

/* 数字输入框样式 */
.number-input {
  display: flex;
  align-items: center;
  gap: calc(8px + (var(--settings-font-scale) - 1) * 2px);
}

.number-button {
  width: calc(28px + (var(--settings-font-scale) - 1) * 6px);
  height: calc(28px + (var(--settings-font-scale) - 1) * 6px);
  border: 1px solid var(--border-color, #d1d9e6);
  background: var(--button-bg, #fff);
  border-radius: 6px;
  font-size: calc(var(--text-lg) * var(--settings-font-scale) * 0.85);
  color: var(--text-primary, #2d3748);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s ease;
  font-weight: 600;
  text-shadow: 0 0.5px 1px rgba(0, 0, 0, 0.1);
  font-feature-settings: 'kern' 1;
}

.number-button:hover {
  background: var(--hover-bg, rgba(0, 0, 0, 0.05));
}

.number-input input {
  width: calc(60px + (var(--settings-font-scale) - 1) * 10px);
  height: calc(28px + (var(--settings-font-scale) - 1) * 6px);
  border: 1px solid var(--border-color, #d1d9e6);
  border-radius: 6px;
  text-align: center;
  font-size: calc(var(--text-base) * var(--settings-font-scale) * 0.9);
  color: var(--text-primary, #2d3748);
  background: var(--input-bg, #fff);
  font-weight: 500;
  font-feature-settings: 'kern' 1, 'tnum' 1;
}

/* 快捷键输入框样式 */
.shortcut-input {
  min-width: calc(120px + (var(--settings-font-scale) - 1) * 20px);
  height: calc(32px + (var(--settings-font-scale) - 1) * 6px);
  border: 1px solid var(--border-color, #d1d9e6);
  border-radius: 6px;
  padding: 0 calc(12px + (var(--settings-font-scale) - 1) * 3px);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: calc(var(--text-base) * var(--settings-font-scale) * 0.9);
  color: var(--text-primary, #2d3748);
  background: var(--input-bg, #fff);
  cursor: pointer;
  transition: all 0.2s ease;
  user-select: none;
  position: relative;
  font-weight: 500;
  font-feature-settings: 'kern' 1;
  font-family: 'SF Mono', Monaco, 'Cascadia Code', 'Roboto Mono', Consolas, 'Courier New', monospace;
}

.shortcut-input:hover {
  border-color: var(--primary-color, #2c7a7b);
}

.shortcut-input.recording {
  border-color: var(--primary-color, #2c7a7b);
  background: var(--input-focus-bg, #f0f9f9);
  box-shadow: 0 0 0 2px rgba(44, 122, 123, 0.1);
}

.shortcut-input.error {
  border-color: var(--error-color, #e53e3e);
  background: var(--error-bg, #fed7d7);
}

.recording-text {
  color: var(--primary-color, #2c7a7b);
  animation: pulse 1.5s infinite;
}

.error-icon {
  margin-left: calc(8px + (var(--settings-font-scale) - 1) * 2px);
  font-size: calc(var(--text-base) * var(--settings-font-scale));
}

.error-message {
  font-size: calc(var(--text-sm) * var(--settings-font-scale) * 0.9);
  color: var(--error-color, #e53e3e);
  margin-top: calc(4px + (var(--settings-font-scale) - 1) * 1px);
  width: 100%;
  line-height: 1.3;
  font-weight: 500;
  font-feature-settings: 'kern' 1;
}

@keyframes pulse {
  0% {
    opacity: 1;
  }

  50% {
    opacity: 0.5;
  }

  100% {
    opacity: 1;
  }
}

.settings-footer {
  padding: var(--spacing-lg) var(--spacing-xl);
  border-top: var(--border-width) solid var(--border-color);
  display: flex;
  justify-content: flex-end;
  gap: var(--spacing-md);
}

.cancel-button,
.confirm-button {
  padding: var(--spacing-sm) var(--spacing-xl);
  border-radius: var(--radius-md);
  font-size: calc(var(--text-base) * var(--settings-font-scale));
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
  letter-spacing: 0.2px;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.1);
  font-feature-settings: 'kern' 1;
}

.cancel-button {
  background: var(--button-bg, #fff);
  border: 1px solid var(--border-color, #d1d9e6);
  color: var(--text-primary, #2d3748);
}

.confirm-button {
  background: var(--primary-color, #2c7a7b);
  border: none;
  color: white;
}

.confirm-button:disabled {
  background: var(--disabled-bg, #a0aec0);
  cursor: not-allowed;
}

.cancel-button:hover:not(:disabled) {
  background: var(--hover-bg, rgba(0, 0, 0, 0.05));
}

.confirm-button:hover:not(:disabled) {
  background: var(--primary-hover, #256d6d);
}

/* 警告框样式 */
.settings-warning {
  display: flex;
  gap: calc(10px + (var(--settings-font-scale) - 1) * 2px);
  margin-top: calc(8px + (var(--settings-font-scale) - 1) * 2px);
  padding: calc(10px + (var(--settings-font-scale) - 1) * 2px) calc(12px + (var(--settings-font-scale) - 1) * 3px);
  background: var(--warning-bg, #fffaf0);
  border: 1px solid var(--warning-border, #f6e05e);
  border-radius: calc(8px + (var(--settings-font-scale) - 1) * 2px);
  width: 100%;
  max-width: 100%;
  box-sizing: border-box;
  overflow: hidden;
  animation: warning-in 0.3s ease;
}

@keyframes warning-in {
  from {
    opacity: 0;
    transform: translateY(-10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.warning-icon {
  font-size: calc(var(--text-sm) * var(--settings-font-scale));
  line-height: 1;
  margin-top: calc(1px + (var(--settings-font-scale) - 1) * 0.5px);
  flex-shrink: 0;
}

.warning-content {
  flex: 1;
  min-width: 0;
}

.warning-title {
  font-size: calc(var(--text-base) * var(--settings-font-scale) * 0.9);
  font-weight: 600;
  color: var(--warning-title, #b7791f);
  margin-bottom: calc(3px + (var(--settings-font-scale) - 1) * 1px);
  letter-spacing: 0.2px;
  font-feature-settings: 'kern' 1;
}

.warning-text {
  font-size: calc(var(--text-sm) * var(--settings-font-scale) * 1);
  line-height: 1.4;
  color: var(--warning-text, #975a16);
  word-wrap: break-word;
  word-break: break-word;
  hyphens: auto;
  font-weight: 400;
  font-feature-settings: 'kern' 1;
}

/* 设置弹窗特定的响应式优化 */
/* 基础响应式已在 responsive.css 中处理，这里只做设置弹窗特殊调整 */

/* 不同屏幕尺寸下的字体放大系数调整 */
.bp-xs.settings-dialog {
  --settings-font-scale: 1.35; /* 极小屏幕字体放大35% */
}

.bp-sm.settings-dialog {
  --settings-font-scale: 1.3; /* 小屏幕字体放大30% */
}

.bp-md.settings-dialog {
  --settings-font-scale: 1.25; /* 中等屏幕字体放大25% */
}

/* 极小窗口下的布局调整 */
.bp-xs .settings-item {
  flex-direction: column;
  align-items: flex-start;
  gap: var(--spacing-sm);
}

.bp-xs .settings-label {
  width: 100%;
}

.bp-xs .switch,
.bp-xs .number-input,
.bp-xs .shortcut-input {
  align-self: flex-end;
}

/* 小窗口下的间距优化 */
.bp-sm .settings-content {
  padding: var(--spacing-lg) var(--spacing-xl);
}

.bp-sm .settings-group {
  gap: var(--spacing-md);
}

/* Windows平台特殊优化 */
@media (-ms-high-contrast: none), (-ms-high-contrast: active) {
  .settings-overlay {
    backdrop-filter: none;
    background: rgba(0, 0, 0, 0.6);
  }
  
  .settings-dialog {
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
  }
}

/* macOS平台特殊优化 */
@supports (-webkit-backdrop-filter: blur()) {
  @media (max-width: 480px) {
    .settings-overlay {
      backdrop-filter: blur(8px);
    }
    
    .settings-dialog {
      backdrop-filter: blur(20px);
      background: rgba(245, 247, 250, 0.95);
    }
  }
}

/* 高DPI显示器优化 */
@media (-webkit-min-device-pixel-ratio: 2), (min-resolution: 192dpi) {
  .settings-dialog {
    border: 0.5px solid transparent;
  }
  
  .settings-header {
    border-bottom-width: 0.5px;
  }
  
  .settings-footer {
    border-top-width: 0.5px;
  }
  
  .switch .slider {
    border: 0.5px solid transparent;
  }
  
  .number-button,
  .shortcut-input {
    border-width: 0.5px;
  }
  
  .settings-warning {
    border-width: 0.5px;
  }
}

/* 暗色模式支持 */
@media (prefers-color-scheme: dark) {
  .settings-dialog {
    --bg-color: #1a1a1a;
    --border-color: #2d2d2d;
    --text-primary: #e6e6e6;
    --text-secondary: #999999;
    --primary-color: #2c7a7b;
    --primary-hover: #256d6d;
    --switch-bg: #4a4a4a;
    --button-bg: #2d2d2d;
    --input-bg: #2d2d2d;
    --hover-bg: rgba(255, 255, 255, 0.1);
    --error-color: #fc8181;
    --error-bg: #742a2a;
    --disabled-bg: #4a5568;
    --warning-bg: #2d2416;
    --warning-border: #d69e2e;
    --warning-title: #d69e2e;
    --warning-text: #f6e05e;
  }
  
  /* 暗色模式下的响应式优化 */
  @media (max-width: 480px) {
    .settings-dialog {
      background: rgba(26, 26, 26, 0.95);
    }
    
    .settings-overlay {
      background: rgba(0, 0, 0, 0.7);
    }
  }
}

/* VIP相关样式 */
.vip-info-item,
.vip-limits-item {
  border-bottom: 1px solid var(--border-color, #e2e8f0);
  padding-bottom: calc(var(--spacing-md) * var(--settings-font-scale));
}

.vip-status-display {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: calc(var(--spacing-sm) * var(--settings-font-scale));
}

.vip-status {
  display: flex;
  align-items: center;
  gap: calc(var(--spacing-sm) * var(--settings-font-scale));
  flex: 1;
}

.vip-icon {
  font-size: calc(var(--text-lg) * var(--settings-font-scale));
}

.vip-type {
  font-weight: 600;
  color: var(--text-primary);
}

.vip-remaining {
  color: var(--text-secondary);
  font-size: calc(var(--text-sm) * var(--settings-font-scale));
}

.vip-status.is-vip .vip-type {
  color: var(--primary-color);
}

.upgrade-button {
  padding: calc(var(--spacing-xs) * var(--settings-font-scale)) calc(var(--spacing-sm) * var(--settings-font-scale));
  border: 1px solid var(--primary-color);
  border-radius: calc(4px * var(--settings-font-scale));
  background: transparent;
  color: var(--primary-color);
  font-size: calc(var(--text-sm) * var(--settings-font-scale));
  cursor: pointer;
  transition: all 0.2s ease;
}

.upgrade-button:hover {
  background: var(--primary-color);
  color: white;
}

.vip-limits-display {
  display: flex;
  flex-direction: column;
  gap: calc(var(--spacing-xs) * var(--settings-font-scale));
}

.limit-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: calc(var(--text-sm) * var(--settings-font-scale));
}

.limit-label {
  color: var(--text-secondary);
}

.limit-value {
  color: var(--text-primary);
  font-weight: 500;
}

.limit-value.vip-feature {
  color: var(--primary-color);
  font-weight: 600;
}
</style>