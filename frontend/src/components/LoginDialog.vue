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
            <label for="register-nickname">昵称 *</label>
            <input
              id="register-nickname"
              v-model="registerForm.nickname"
              type="text"
              placeholder="请输入昵称"
              required
              :disabled="isLoading"
              :class="{ 'error': fieldValidation.nickname.error }"
            />
            <div v-if="fieldValidation.nickname.error" class="field-error">
              {{ fieldValidation.nickname.error }}
            </div>
          </div>

          <div class="form-group">
            <label for="register-account">用户名 *</label>
            <input
              id="register-account"
              v-model="registerForm.account"
              type="text"
              placeholder="请输入用户名（至少3位）"
              required
              :disabled="isLoading"
              :class="{ 'error': fieldValidation.account.error }"
            />
            <div v-if="fieldValidation.account.error" class="field-error">
              {{ fieldValidation.account.error }}
            </div>
          </div>
          
          <div class="form-group">
            <label for="register-password">密码 *</label>
            <input
              id="register-password"
              v-model="registerForm.password"
              type="password"
              placeholder="请输入密码（至少6位）"
              required
              :disabled="isLoading"
              :class="{ 'error': fieldValidation.password.error }"
            />
            <div v-if="fieldValidation.password.error" class="field-error">
              {{ fieldValidation.password.error }}
            </div>
          </div>
          
          <div class="form-group">
            <label for="register-confirm-password">确认密码 *</label>
            <input
              id="register-confirm-password"
              v-model="registerForm.confirmPassword"
              type="password"
              placeholder="请再次输入密码"
              required
              :disabled="isLoading"
              :class="{ 'error': fieldValidation.confirmPassword.error }"
            />
            <div v-if="fieldValidation.confirmPassword.error" class="field-error">
              {{ fieldValidation.confirmPassword.error }}
            </div>
          </div>

          <div class="form-group">
            <label for="register-email">邮箱 *</label>
            <input
              id="register-email"
              v-model="registerForm.email"
              type="email"
              placeholder="请输入邮箱地址"
              required
              :disabled="isLoading"
              :class="{ 'error': fieldValidation.email.error }"
            />
            <div v-if="fieldValidation.email.error" class="field-error">
              {{ fieldValidation.email.error }}
            </div>
          </div>

          <div class="form-group email-captcha-group">
            <label for="register-captcha">邮箱验证码 *</label>
            <div class="captcha-input-group">
              <input
                id="register-captcha"
                v-model="registerForm.captcha"
                type="text"
                placeholder="请输入验证码"
                required
                :disabled="isLoading"
                :class="{ 'error': fieldValidation.captcha.error }"
              />
              <button
                type="button"
                class="captcha-btn"
                :disabled="isLoading || isCaptchaLoading || captchaCountdown > 0"
                @click="sendEmailCaptcha"
              >
                {{ captchaButtonText }}
              </button>
            </div>
            <div v-if="fieldValidation.captcha.error" class="field-error">
              {{ fieldValidation.captcha.error }}
            </div>
          </div>

          <div class="form-group">
            <label for="register-phone">手机号</label>
            <input
              id="register-phone"
              v-model="registerForm.phone"
              type="tel"
              placeholder="请输入手机号（可选）"
              :disabled="isLoading"
            />
          </div>
          
          <button type="submit" class="submit-btn" :disabled="isLoading || !isRegisterFormValid">
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
import { ref, reactive, inject, watch, computed } from 'vue'
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
  nickname: '',
  account: '',
  password: '',
  confirmPassword: '',
  email: '',
  captcha: '',
  phone: ''
})

// 邮箱验证码相关
const isCaptchaLoading = ref(false)
const captchaCountdown = ref(0)
let countdownTimer: ReturnType<typeof setInterval> | null = null

// 计算属性
const captchaButtonText = computed(() => {
  if (isCaptchaLoading.value) return '发送中...'
  if (captchaCountdown.value > 0) return `${captchaCountdown.value}秒后重发`
  return '发送验证码'
})

// 字段验证功能已通过计算属性实现

// 字段验证计算属性
const fieldValidation = computed(() => ({
  nickname: {
    isValid: registerForm.nickname.trim().length > 0,
    error: registerForm.nickname.trim().length === 0 && registerForm.nickname !== '' ? '请输入昵称' : ''
  },
  account: {
    isValid: registerForm.account.trim().length >= 3,
    error: registerForm.account.trim().length > 0 && registerForm.account.trim().length < 3 ? '用户名至少需要3个字符' : 
           registerForm.account.trim().length === 0 && registerForm.account !== '' ? '请输入用户名' : ''
  },
  password: {
    isValid: registerForm.password.length >= 6,
    error: registerForm.password.length > 0 && registerForm.password.length < 6 ? '密码至少需要6个字符' : 
           registerForm.password.length === 0 && registerForm.password !== '' ? '请输入密码' : ''
  },
  confirmPassword: {
    isValid: registerForm.confirmPassword && registerForm.password === registerForm.confirmPassword,
    error: registerForm.confirmPassword && registerForm.password !== registerForm.confirmPassword ? '两次输入的密码不一致' :
           registerForm.confirmPassword === '' && registerForm.password !== '' ? '请确认密码' : ''
  },
  email: {
    isValid: registerForm.email.trim() && isValidEmail(registerForm.email),
    error: registerForm.email.trim() && !isValidEmail(registerForm.email) ? '请输入有效的邮箱地址' :
           registerForm.email.trim() === '' && registerForm.email !== '' ? '请输入邮箱地址' : ''
  },
  captcha: {
    isValid: registerForm.captcha.trim().length > 0,
    error: registerForm.captcha.trim().length === 0 && registerForm.captcha !== '' ? '请输入邮箱验证码' : ''
  }
}))

const isRegisterFormValid = computed(() => {
  return fieldValidation.value.nickname.isValid &&
         fieldValidation.value.account.isValid &&
         fieldValidation.value.password.isValid &&
         fieldValidation.value.confirmPassword.isValid &&
         fieldValidation.value.email.isValid &&
         fieldValidation.value.captcha.isValid
})

// 工具函数
const isValidEmail = (email: string): boolean => {
  const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
  return emailRegex.test(email)
}

const close = () => {
  emit('update:visible', false)
  // 清除倒计时
  if (countdownTimer) {
    clearInterval(countdownTimer)
    countdownTimer = null
  }
  captchaCountdown.value = 0
  // 重置表单
  resetForms()
}

const handleOverlayClick = () => {
  close()
}

const resetForms = () => {
  loginForm.account = ''
  loginForm.password = ''
  registerForm.nickname = ''
  registerForm.account = ''
  registerForm.password = ''
  registerForm.confirmPassword = ''
  registerForm.email = ''
  registerForm.captcha = ''
  registerForm.phone = ''
  currentView.value = 'login'
  isLoading.value = false
  isCaptchaLoading.value = false
}

const switchToRegister = () => {
  currentView.value = 'register'
}

const switchToLogin = () => {
  currentView.value = 'login'
}

// 发送邮箱验证码
const sendEmailCaptcha = async () => {
  if (!registerForm.email.trim()) {
    showMessageBar('请先输入邮箱地址', 'warning')
    return
  }
  
  if (!isValidEmail(registerForm.email)) {
    showMessageBar('请输入有效的邮箱地址', 'warning')
    return
  }
  
  isCaptchaLoading.value = true
  
  try {
    // TODO: 这里需要添加发送验证码的API调用
    // 暂时模拟发送验证码
    await new Promise(resolve => setTimeout(resolve, 1000))
    
    showMessageBar('验证码已发送到您的邮箱，请查收', 'info')
    
    // 开始倒计时
    captchaCountdown.value = 60
    countdownTimer = setInterval(() => {
      captchaCountdown.value--
      if (captchaCountdown.value <= 0) {
        if (countdownTimer) {
          clearInterval(countdownTimer)
          countdownTimer = null
        }
      }
    }, 1000)
    
  } catch (error) {
    console.error('发送验证码失败:', error)
    showMessageBar('发送验证码失败，请稍后重试', 'error')
  } finally {
    isCaptchaLoading.value = false
  }
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
  
  // 详细的表单验证
  if (!registerForm.nickname.trim()) {
    showMessageBar('请输入昵称', 'warning')
    return
  }
  
  if (!registerForm.account.trim() || registerForm.account.trim().length < 3) {
    showMessageBar('用户名至少需要3个字符', 'warning')
    return
  }
  
  if (!registerForm.password || registerForm.password.length < 6) {
    showMessageBar('密码至少需要6个字符', 'warning')
    return
  }
  
  if (!registerForm.confirmPassword) {
    showMessageBar('请确认密码', 'warning')
    return
  }
  
  if (registerForm.password !== registerForm.confirmPassword) {
    showMessageBar('两次输入的密码不一致', 'warning')
    return
  }
  
  if (!registerForm.email.trim()) {
    showMessageBar('请输入邮箱地址', 'warning')
    return
  }
  
  if (!isValidEmail(registerForm.email)) {
    showMessageBar('请输入有效的邮箱地址', 'warning')
    return
  }
  
  if (!registerForm.captcha.trim()) {
    showMessageBar('请输入邮箱验证码', 'warning')
    return
  }
  
  try {
    isLoading.value = true
    
    const response = await userApi.register({
      nickname: registerForm.nickname.trim(),
      account: registerForm.account.trim(),
      password: registerForm.password,
      confirmPassword: registerForm.confirmPassword,
      email: registerForm.email.trim(),
      captcha: registerForm.captcha.trim(),
      phone: registerForm.phone.trim() || undefined
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
  max-width: 380px;
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
  padding: 16px 20px 0 20px;
  border-bottom: none;
}

.form-title {
  margin: 0;
  font-size: 20px;
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
  padding: 12px 20px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.form-group label {
  font-weight: 500;
  color: var(--text-primary, #333);
  font-size: 14px;
}

.form-group input {
  padding: 10px 12px;
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
  padding: 10px;
  background: linear-gradient(135deg, var(--header-bg, #2c7a7b), #319795);
  color: white;
  border: none;
  border-radius: var(--radius-md, 8px);
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-height: 42px;
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
  padding: 8px 20px 16px;
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

/* 验证码相关样式 */
.email-captcha-group .captcha-input-group {
  display: flex;
  gap: 8px;
  align-items: center;
}

.email-captcha-group input {
  flex: 1;
  margin-bottom: 0;
}

.captcha-btn {
  background: var(--primary-color, #2c7a7b);
  color: white;
  border: none;
  border-radius: var(--radius-md, 8px);
  padding: 0 12px;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
  white-space: nowrap;
  min-width: 80px;
  height: 34px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.captcha-btn:hover:not(:disabled) {
  background: var(--primary-hover, #319795);
  transform: translateY(-1px);
}

.captcha-btn:disabled {
  background: var(--bg-disabled, #e2e8f0);
  color: var(--text-disabled, #a0a0a0);
  cursor: not-allowed;
  transform: none;
}

/* 字段验证样式 */
.form-group input.error {
  border-color: #ef4444 !important;
  box-shadow: 0 0 0 1px rgba(239, 68, 68, 0.1) !important;
}

.form-group input.error:focus {
  border-color: #ef4444 !important;
  box-shadow: 0 0 0 3px rgba(239, 68, 68, 0.1) !important;
}

.field-error {
  color: #ef4444;
  font-size: 11px;
  margin-top: 2px;
  display: flex;
  align-items: center;
  animation: slideInError 0.2s ease;
  min-height: 14px; /* 固定最小高度 */
  line-height: 1.2;
}

@keyframes slideInError {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

/* 响应式适配 */
@media (max-width: 768px) {
  .login-container {
    max-width: 500px;
    margin: 15px;
    max-height: 85vh;
  }
  
  .form-title {
    font-size: 22px;
  }
  
  .captcha-btn {
    min-width: 90px;
    font-size: 13px;
    padding: 0 12px;
  }
  
  .captcha-input-group {
    gap: 6px;
  }
}

@media (max-width: 480px) {
  .login-container {
    margin: 10px;
    max-width: none;
    border-radius: var(--radius-md, 8px);
  }
  
  .form-header, .form-content, .form-footer {
    padding-left: 16px;
    padding-right: 16px;
  }
  
  .form-header {
    padding-top: 20px;
  }
  
  .form-title {
    font-size: 20px;
  }
  
  .form-group input {
    font-size: 16px; /* 防止iOS缩放 */
    padding: 10px 12px;
  }
  
  .captcha-btn {
    min-width: 70px;
    font-size: 11px;
    padding: 0 8px;
    height: 32px;
  }
  
  .captcha-input-group {
    gap: 4px;
  }
  
  .field-error {
    font-size: 10px;
  }
}

/* 只在极低屏幕高度时添加滚动 */
@media (max-height: 500px) {
  .login-container {
    max-height: 95vh;
    overflow-y: auto;
  }
}

@media (max-width: 360px) {
  .login-container {
    margin: 5px;
    border-radius: var(--radius-sm, 6px);
    max-height: 95vh;
    overflow-y: auto;
  }
  
  .form-header, .form-content, .form-footer {
    padding-left: 12px;
    padding-right: 12px;
  }
  
  .form-title {
    font-size: 18px;
  }
  
  .form-group input {
    padding: 10px 12px;
    font-size: 16px;
  }
  
  .captcha-input-group {
    flex-direction: column;
    gap: 8px;
  }
  
  .captcha-btn {
    width: 100%;
    min-width: auto;
    height: 36px;
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