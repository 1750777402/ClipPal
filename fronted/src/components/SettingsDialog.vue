<template>
  <div v-if="modelValue" class="settings-overlay" @click.self="handleClose">
    <div class="settings-dialog">
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
              <input type="number" v-model.number="settings.max_records" min="50" max="1000">
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
                <span>{{ settings.shortcut_key || '点击设置' }}</span>
                <span v-if="shortcutError" class="error-icon">⚠️</span>
              </template>
            </div>
            <div v-if="shortcutError" class="error-message">{{ shortcutError }}</div>
          </div>

          <div class="settings-item">
            <div class="settings-label">
              <span>云同步</span>
              <span class="settings-description">自动同步剪贴板内容到云端(该功能当前未启用)</span>
            </div>
            <label class="switch">
              <input type="checkbox" :checked="settings.cloud_sync === 1" @change="(e: Event) => settings.cloud_sync = (e.target as HTMLInputElement).checked ? 1 : 0">
              <span class="slider"></span>
            </label>
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
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onBeforeUnmount, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';

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
}

const settings = ref<Settings>({
  auto_start: 0,
  max_records: 200,
  shortcut_key: 'Ctrl+`',
  cloud_sync: 0
});

const isRecording = ref(false);
const isSaving = ref(false);
const shortcutError = ref('');
const pressedKeys = ref<string[]>([]);

// 计算是否有错误
const hasErrors = computed(() => {
  return shortcutError.value !== '' || 
         settings.value.max_records < 50 || 
         settings.value.max_records > 1000;
});

// 监听弹窗打开时加载设置
watch(() => props.modelValue, async (newVal) => {
  if (newVal) {
    try {
      const currentSettings = await invoke('load_settings') as Settings;
      console.log('当前设置:', currentSettings);
      settings.value = { ...currentSettings };
      // 清除错误状态
      shortcutError.value = '';
    } catch (error) {
      console.error('加载设置失败:', error);
    }
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
    await invoke('save_settings', { settings: settings.value });
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
    const isValid = await invoke('validate_shortcut', { shortcut }) as boolean;
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
  const modifiers = ['Ctrl', 'Shift', 'Alt', 'Meta'].filter(mod => {
    if (mod === 'Ctrl' && e.ctrlKey) return true;
    if (mod === 'Shift' && e.shiftKey) return true;
    if (mod === 'Alt' && e.altKey) return true;
    if (mod === 'Meta' && e.metaKey) return true;
    return false;
  });

  // 2. 处理普通键（映射特殊键，保证可读性）
  let key = e.key;
  const keyMap: { [k: string]: string } = {
    ' ': 'Space',
    'Escape': 'Esc',
    'ArrowUp': '↑',
    'ArrowDown': '↓',
    'ArrowLeft': '←',
    'ArrowRight': '→',
    'Backspace': 'Backspace',
    'Delete': 'Delete',
    'Enter': 'Enter',
    'Tab': 'Tab',
    'CapsLock': 'CapsLock',
    'Insert': 'Insert',
    'Home': 'Home',
    'End': 'End',
    'PageUp': 'PageUp',
    'PageDown': 'PageDown',
    'PrintScreen': 'PrintScreen',
    'ScrollLock': 'ScrollLock',
    'Pause': 'Pause',
    'ContextMenu': 'Menu',
    'NumLock': 'NumLock',
    'Backquote': '`',
    'Control': 'Ctrl'
  };
  key = keyMap[key] || key; // 特殊键映射
  if (key.length === 1) key = key.toUpperCase(); // 单个字符转大写

  // 3. 合并修饰键与普通键，去重并保持顺序
  const newKeys = [...modifiers, key];
  newKeys.forEach(k => {
    if (!pressedKeys.value.includes(k)) { // 去重：已存在则不重复添加
      pressedKeys.value.push(k);
    }
  });

  // 4. 限制最大按键数（最多4个，避免无意义组合）
  if (pressedKeys.value.length > 4) {
    pressedKeys.value.shift(); // 超过则移除最早按下的键
  }

  // 5. 保存条件：至少1个修饰键 + 1个普通键
  const hasModifier = modifiers.length > 0;
  const hasRegularKey = !['Ctrl', 'Shift', 'Alt', 'Meta'].includes(key);
  if (hasModifier && hasRegularKey) {
    const newShortcut = pressedKeys.value.join('+'); // 按顺序拼接
    settings.value.shortcut_key = newShortcut;
    // 实时验证快捷键
    const isValid = await validateShortcut(newShortcut);
    if (isValid) {
      stopRecording(); // 录制完成
    }
  }
};

// 键释放时精准移除（含修饰键状态检查）
const handleKeyUp = (e: KeyboardEvent) => {
  if (!isRecording.value) return;

  // 1. 处理释放的键（映射特殊键）
  let key = e.key;
  const keyMap: { [k: string]: string } = {
    ' ': 'Space',
    'Escape': 'Esc',
    'ArrowUp': '↑',
    'ArrowDown': '↓',
    'ArrowLeft': '←',
    'ArrowRight': '→',
    'Backspace': 'Backspace',
    'Delete': 'Delete',
    'Enter': 'Enter',
    'Tab': 'Tab',
    'CapsLock': 'CapsLock',
    'Insert': 'Insert',
    'Home': 'Home',
    'End': 'End',
    'PageUp': 'PageUp',
    'PageDown': 'PageDown',
    'PrintScreen': 'PrintScreen',
    'ScrollLock': 'ScrollLock',
    'Pause': 'Pause',
    'ContextMenu': 'Menu',
    'NumLock': 'NumLock',
    'Backquote': '`',
    'Control': 'Ctrl'
  };
  key = keyMap[key] || key;
  if (key.length === 1) key = key.toUpperCase();

  // 2. 从数组中移除释放的键
  const index = pressedKeys.value.indexOf(key);
  if (index > -1) {
    pressedKeys.value.splice(index, 1);
  }

  // 3. 处理修饰键释放（即使释放的不是修饰键，也要检查状态）
  ['Ctrl', 'Shift', 'Alt', 'Meta'].forEach(mod => {
    const isReleased = mod === 'Ctrl' ? !e.ctrlKey :
      mod === 'Shift' ? !e.shiftKey :
        mod === 'Alt' ? !e.altKey :
          mod === 'Meta' ? !e.metaKey : false;
    if (isReleased) {
      const modIndex = pressedKeys.value.indexOf(mod);
      if (modIndex > -1) {
        pressedKeys.value.splice(modIndex, 1);
      }
    }
  });
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

onMounted(() => {
  window.addEventListener('keydown', handleKeyDown);
  window.addEventListener('keyup', handleKeyUp);
  window.addEventListener('click', handleClickOutside);
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeyDown);
  window.removeEventListener('keyup', handleKeyUp);
  window.removeEventListener('click', handleClickOutside);
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
}

.settings-dialog {
  background: var(--bg-color, #f5f7fa);
  border-radius: 16px;
  width: 90%;
  max-width: 400px;
  box-shadow: 0 4px 24px rgba(0, 0, 0, 0.15);
  display: flex;
  flex-direction: column;
  animation: dialog-in 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

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
  padding: 16px 20px;
  border-bottom: 1px solid var(--border-color, #d1d9e6);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.settings-title {
  margin: 0;
  font-size: 18px;
  color: var(--text-primary, #2d3748);
  font-weight: 600;
}

.close-button {
  background: none;
  border: none;
  font-size: 24px;
  color: var(--text-secondary, #666);
  cursor: pointer;
  padding: 4px;
  line-height: 1;
  border-radius: 4px;
  transition: all 0.2s ease;
}

.close-button:hover {
  background: var(--hover-bg, rgba(0, 0, 0, 0.05));
  color: var(--text-primary, #2d3748);
}

.settings-content {
  padding: 20px;
  overflow-y: auto;
  max-height: calc(100vh - 200px);
}

.settings-group {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.settings-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
  flex-wrap: wrap;
}

.settings-label {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 1;
  min-width: 0;
}

.settings-label span:first-child {
  font-weight: 500;
  color: var(--text-primary, #2d3748);
}

.settings-description {
  font-size: 12px;
  color: var(--text-secondary, #666);
}

/* 开关样式 */
.switch {
  position: relative;
  display: inline-block;
  width: 44px;
  height: 24px;
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
  height: 20px;
  width: 20px;
  left: 2px;
  bottom: 2px;
  background-color: white;
  transition: .3s;
  border-radius: 50%;
}

input:checked+.slider {
  background-color: var(--primary-color, #2c7a7b);
}

input:checked+.slider:before {
  transform: translateX(20px);
}

/* 数字输入框样式 */
.number-input {
  display: flex;
  align-items: center;
  gap: 8px;
}

.number-button {
  width: 28px;
  height: 28px;
  border: 1px solid var(--border-color, #d1d9e6);
  background: var(--button-bg, #fff);
  border-radius: 6px;
  font-size: 16px;
  color: var(--text-primary, #2d3748);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s ease;
}

.number-button:hover {
  background: var(--hover-bg, rgba(0, 0, 0, 0.05));
}

.number-input input {
  width: 60px;
  height: 28px;
  border: 1px solid var(--border-color, #d1d9e6);
  border-radius: 6px;
  text-align: center;
  font-size: 14px;
  color: var(--text-primary, #2d3748);
  background: var(--input-bg, #fff);
}

/* 快捷键输入框样式 */
.shortcut-input {
  min-width: 120px;
  height: 32px;
  border: 1px solid var(--border-color, #d1d9e6);
  border-radius: 6px;
  padding: 0 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  color: var(--text-primary, #2d3748);
  background: var(--input-bg, #fff);
  cursor: pointer;
  transition: all 0.2s ease;
  user-select: none;
  position: relative;
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
  margin-left: 8px;
  font-size: 16px;
}

.error-message {
  font-size: 12px;
  color: var(--error-color, #e53e3e);
  margin-top: 4px;
  width: 100%;
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
  padding: 16px 20px;
  border-top: 1px solid var(--border-color, #d1d9e6);
  display: flex;
  justify-content: flex-end;
  gap: 12px;
}

.cancel-button,
.confirm-button {
  padding: 8px 20px;
  border-radius: 8px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
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
  }
}
</style>