<template>
  <div v-if="visible" class="login-overlay" @click="handleOverlayClick">
    <div class="login-container" @click.stop>
      <!-- 登录窗口 -->
      <div v-if="currentView === 'login'" class="login-form">
        <div class="form-header">
          <h2 class="form-title">账户登录</h2>
          <button class="close-btn" @click="close" type="button">×</button>
        </div>
        
        <form @submit.prevent="handleLogin" class="form-content">
          <div class="form-group">
            <label for="login-account">账号</label>
            <input
              id="login-account"
              v-model="loginForm.account"
              type="text"
              placeholder="请输入账号"
              required
              :disabled="isLoading"
              @keypress.enter="handleLogin"
            />
          </div>
          
          <div class="form-group">
            <label for="login-password">密码</label>
            <input
              id="login-password"
              v-model="loginForm.password"
              type="password"
              placeholder="请输入密码"
              required
              :disabled="isLoading"
              @keypress.enter="handleLogin"
            />
          </div>
          
          <button type="submit" class="submit-btn" :disabled="isLoading">
            <span v-if="isLoading" class="loading-spinner"></span>
            {{ isLoading ? '登录中...' : '登录' }}
          </button>
        </form>
        
        <div class="form-footer">
          <span>还没有账号？</span>
          <button type="button" class="link-btn" @click="switchToRegister">注册</button>
        </div>
      </div>
      
      <!-- 注册窗口 -->
      <div v-if="currentView === 'register'" class="register-form">
        <div class="form-header">
          <h2 class="form-title">账户注册</h2>
          <button class="close-btn" @click="close" type="button">×</button>
        </div>
        
        <form @submit.prevent="handleRegister" class="form-content">
          <div class="form-group">
            <label for="register-account">账号</label>
            <input
              id="register-account"
              v-model="registerForm.account"
              type="text"
              placeholder="请输入账号（至少3位）"
              required
              :disabled="isLoading"
              @keypress.enter="handleRegister"
            />
          </div>
          
          <div class="form-group">
            <label for="register-password">密码</label>
            <input
              id="register-password"
              v-model="registerForm.password"
              type="password"
              placeholder="请输入密码（至少6位）"
              required
              :disabled="isLoading"
              @keypress.enter="handleRegister"
            />
          </div>
          
          <div class="form-group">
            <label for="register-confirm-password">确认密码</label>
            <input
              id="register-confirm-password"
              v-model="registerForm.confirmPassword"
              type="password"
              placeholder="请再次输入密码"
              required
              :disabled="isLoading"
              @keypress.enter="handleRegister"
            />
          </div>
          
          <button type="submit" class="submit-btn" :disabled="isLoading">
            <span v-if="isLoading" class="loading-spinner"></span>
            {{ isLoading ? '注册中...' : '注册' }}
          </button>
        </form>
        
        <div class="form-footer">
          <span>已有账号？</span>
          <button type="button" class="link-btn" @click="switchToLogin">登录</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, inject, watch } from 'vue'
import { userApi, isSuccess } from '../utils/api'

interface Props {
  visible: boolean
}

interface Emits {
  (e: 'update:visible', value: boolean): void
  (e: 'login-success', user: any): void
}

const props = defineProps<Props>()
const emit = defineEmits<Emits>()

const showMessageBar = inject('showMessageBar') as (message: string, type?: 'info' | 'warning' | 'error') => void

const currentView = ref<'login' | 'register'>('login')
const isLoading = ref(false)

const loginForm = reactive({
  account: '',
  password: ''
})

const registerForm = reactive({
  account: '',
  password: '',
  confirmPassword: ''
})

const close = () => {
  emit('update:visible', false)
  // 重置表单
  resetForms()
}

const handleOverlayClick = () => {
  close()
}

const resetForms = () => {
  loginForm.account = ''
  loginForm.password = ''
  registerForm.account = ''
  registerForm.password = ''
  registerForm.confirmPassword = ''
  currentView.value = 'login'
  isLoading.value = false
}

const switchToRegister = () => {
  currentView.value = 'register'
}

const switchToLogin = () => {
  currentView.value = 'login'
}

const handleLogin = async () => {
  if (isLoading.value) return
  
  // 简单的表单验证
  if (!loginForm.account.trim()) {
    showMessageBar('请输入账号', 'warning')
    return
  }
  
  if (!loginForm.password) {
    showMessageBar('请输入密码', 'warning')
    return
  }
  
  try {
    isLoading.value = true
    
    const response = await userApi.login({
      account: loginForm.account.trim(),
      password: loginForm.password
    })
    
    if (isSuccess(response)) {
      showMessageBar('登录成功！', 'info')
      emit('login-success', response.data)
      close()
    }
  } catch (error) {
    console.error('登录失败:', error)
  } finally {
    isLoading.value = false
  }
}

const handleRegister = async () => {
  if (isLoading.value) return
  
  // 表单验证
  if (!registerForm.account.trim()) {
    showMessageBar('请输入账号', 'warning')
    return
  }
  
  if (!registerForm.password) {
    showMessageBar('请输入密码', 'warning')
    return
  }
  
  if (!registerForm.confirmPassword) {
    showMessageBar('请确认密码', 'warning')
    return
  }
  
  // 验证密码一致性
  if (registerForm.password !== registerForm.confirmPassword) {
    showMessageBar('两次输入的密码不一致', 'warning')
    return
  }
  
  // 简单的密码强度验证
  if (registerForm.password.length < 6) {
    showMessageBar('密码长度不能少于6位', 'warning')
    return
  }
  
  // 账号长度验证
  if (registerForm.account.trim().length < 3) {
    showMessageBar('账号长度不能少于3位', 'warning')
    return
  }
  
  try {
    isLoading.value = true
    
    const response = await userApi.register({
      account: registerForm.account.trim(),
      password: registerForm.password
    })
    
    if (isSuccess(response)) {
      showMessageBar('注册成功！请登录', 'info')
      switchToLogin()
      // 将注册的账号填入登录表单
      loginForm.account = registerForm.account.trim()
      // 清空密码字段
      loginForm.password = ''
    }
  } catch (error) {
    console.error('注册失败:', error)
  } finally {
    isLoading.value = false
  }
}

// 监听visible变化，重置表单
watch(() => props.visible, (newVisible) => {
  if (newVisible) {
    resetForms()
  }
})
</script>

<style scoped>
.login-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  animation: fadeIn 0.3s ease;
}

.login-container {
  background: var(--card-bg, #ffffff);
  border-radius: var(--radius-lg, 12px);
  box-shadow: 0 20px 40px rgba(0, 0, 0, 0.15);
  width: 100%;
  max-width: 400px;
  margin: 20px;
  animation: slideUp 0.3s ease;
}

.login-form, .register-form {
  padding: 0;
}

.form-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 24px 24px 0 24px;
  border-bottom: none;
}

.form-title {
  margin: 0;
  font-size: 24px;
  font-weight: 600;
  color: var(--text-primary, #333);
}

.close-btn {
  background: none;
  border: none;
  font-size: 24px;
  color: var(--text-secondary, #666);
  cursor: pointer;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  transition: all 0.2s ease;
}

.close-btn:hover {
  background: var(--bg-hover, #f5f5f5);
  color: var(--text-primary, #333);
}

.form-content {
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.form-group label {
  font-weight: 500;
  color: var(--text-primary, #333);
  font-size: 14px;
}

.form-group input {
  padding: 12px 16px;
  border: 2px solid var(--border-color, #e2e8f0);
  border-radius: var(--radius-md, 8px);
  font-size: 14px;
  transition: border-color 0.2s ease;
  background: var(--input-bg, #ffffff);
  color: var(--text-primary, #333);
}

.form-group input:focus {
  outline: none;
  border-color: var(--primary-color, #2c7a7b);
  box-shadow: 0 0 0 3px rgba(44, 122, 123, 0.1);
}

.form-group input:disabled {
  background: var(--bg-disabled, #f7fafc);
  cursor: not-allowed;
}

.submit-btn {
  padding: 12px;
  background: linear-gradient(135deg, var(--header-bg, #2c7a7b), #319795);
  color: white;
  border: none;
  border-radius: var(--radius-md, 8px);
  font-size: 16px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-height: 48px;
}

.submit-btn:hover:not(:disabled) {
  background: linear-gradient(135deg, #319795, #2dd4bf);
  box-shadow: 0 4px 12px rgba(44, 122, 123, 0.25);
  transform: translateY(-1px);
}

.submit-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
  transform: none;
}

.loading-spinner {
  width: 16px;
  height: 16px;
  border: 2px solid rgba(255, 255, 255, 0.3);
  border-top-color: white;
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

.form-footer {
  padding: 16px 24px 24px;
  text-align: center;
  color: var(--text-secondary, #666);
  font-size: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.link-btn {
  background: none;
  border: none;
  color: var(--primary-color, #2c7a7b);
  cursor: pointer;
  font-weight: 500;
  text-decoration: underline;
  transition: color 0.2s ease;
}

.link-btn:hover {
  color: var(--primary-hover, #319795);
}

@keyframes fadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

@keyframes slideUp {
  from {
    opacity: 0;
    transform: translateY(20px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

/* 响应式适配 */
@media (max-width: 480px) {
  .login-container {
    margin: 10px;
    max-width: none;
  }
  
  .form-header, .form-content, .form-footer {
    padding-left: 20px;
    padding-right: 20px;
  }
  
  .form-title {
    font-size: 20px;
  }
}

/* 暗色模式支持 */
@media (prefers-color-scheme: dark) {
  .login-container {
    --card-bg: #2d2d2d;
    --text-primary: #e6e6e6;
    --text-secondary: #999999;
    --border-color: #3d3d3d;
    --input-bg: #333333;
    --bg-hover: #3d3d3d;
    --bg-disabled: #2a2a2a;
    --primary-color: #4a9a9a;
    --primary-hover: #5bb6b6;
  }
}
</style>