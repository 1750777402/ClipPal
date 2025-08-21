import { ref, reactive } from 'vue'
import { userApi, isSuccess } from './api'

export interface UserInfo {
  id: number
  account: string
  nickname?: string
  avatar?: string
  email?: string
  created_at?: string
  last_login_time?: string
}

export interface UserState {
  isLoggedIn: boolean
  userInfo: UserInfo | null
  token: string | null
}

// 创建响应式的用户状态
const userState = reactive<UserState>({
  isLoggedIn: false,
  userInfo: null,
  token: null
})

// 本地存储的键名
const STORAGE_KEYS = {
  TOKEN: 'clip-pal-token',
  USER_INFO: 'clip-pal-user-info'
}

/**
 * 用户状态管理
 */
export const userStore = {
  // 获取用户状态
  getState: () => userState,

  // 检查是否已登录
  isLoggedIn: () => userState.isLoggedIn,

  // 获取用户信息
  getUserInfo: () => userState.userInfo,

  // 获取Token
  getToken: () => userState.token,

  // 设置用户登录状态
  setLoginState: (userInfo: UserInfo, token: string) => {
    userState.isLoggedIn = true
    userState.userInfo = userInfo
    userState.token = token
    
    // 保存到本地存储
    localStorage.setItem(STORAGE_KEYS.TOKEN, token)
    localStorage.setItem(STORAGE_KEYS.USER_INFO, JSON.stringify(userInfo))
  },

  // 清除用户登录状态
  clearLoginState: () => {
    userState.isLoggedIn = false
    userState.userInfo = null
    userState.token = null
    
    // 清除本地存储
    localStorage.removeItem(STORAGE_KEYS.TOKEN)
    localStorage.removeItem(STORAGE_KEYS.USER_INFO)
  },

  // 从本地存储恢复用户状态
  restoreFromStorage: () => {
    try {
      const token = localStorage.getItem(STORAGE_KEYS.TOKEN)
      const userInfoStr = localStorage.getItem(STORAGE_KEYS.USER_INFO)
      
      if (token && userInfoStr) {
        const userInfo = JSON.parse(userInfoStr) as UserInfo
        userState.isLoggedIn = true
        userState.userInfo = userInfo
        userState.token = token
        return true
      }
    } catch (error) {
      console.error('恢复用户状态失败:', error)
      // 清除可能损坏的数据
      userStore.clearLoginState()
    }
    return false
  },

  // 验证Token有效性
  validateToken: async (): Promise<boolean> => {
    if (!userState.token) {
      return false
    }

    try {
      const response = await userApi.validateToken()
      if (isSuccess(response)) {
        // Token有效，可能需要更新用户信息
        if (response.data && response.data.userInfo) {
          userState.userInfo = response.data.userInfo
          localStorage.setItem(STORAGE_KEYS.USER_INFO, JSON.stringify(response.data.userInfo))
        }
        return true
      } else {
        // Token无效，清除登录状态
        console.warn('Token已失效，清除登录状态')
        userStore.clearLoginState()
        return false
      }
    } catch (error) {
      console.error('Token验证失败:', error)
      // 网络错误不清除登录状态，可能只是网络问题
      if (error instanceof Error && error.message.includes('网络')) {
        console.warn('网络错误，保持登录状态')
        return true // 假设Token仍然有效
      }
      // 其他错误（如Token格式错误等）清除登录状态
      userStore.clearLoginState()
      return false
    }
  },

  // 登出
  logout: async (): Promise<boolean> => {
    try {
      // 调用后端登出接口
      const response = await userApi.logout()
      
      // 无论后端是否成功，都清除本地状态
      userStore.clearLoginState()
      
      return isSuccess(response)
    } catch (error) {
      console.error('登出失败:', error)
      // 即使请求失败，也清除本地状态
      userStore.clearLoginState()
      return false
    }
  },

  // 更新用户信息
  updateUserInfo: (userInfo: Partial<UserInfo>) => {
    if (userState.userInfo) {
      Object.assign(userState.userInfo, userInfo)
      localStorage.setItem(STORAGE_KEYS.USER_INFO, JSON.stringify(userState.userInfo))
    }
  },

  // 初始化用户状态（应用启动时调用）
  initialize: async (): Promise<boolean> => {
    try {
      // 先从本地存储恢复状态
      const restored = userStore.restoreFromStorage()
      
      if (restored) {
        console.log('从本地存储恢复用户状态:', userState.userInfo?.account)
        
        // 验证Token是否仍然有效
        const isValid = await userStore.validateToken()
        
        if (isValid) {
          console.log('用户Token验证成功，保持登录状态')
        } else {
          console.log('用户Token验证失败，清除登录状态')
        }
        
        return isValid
      }
      
      console.log('未找到本地用户状态，用户未登录')
      return false
    } catch (error) {
      console.error('用户状态初始化失败:', error)
      // 初始化失败，确保清除可能损坏的状态
      userStore.clearLoginState()
      return false
    }
  },

  // 检查登录状态是否过期
  isTokenExpired: (): boolean => {
    // 这里可以根据Token的过期时间来判断
    // 暂时返回false，实际项目中应该解析Token或记录过期时间
    return false
  }
}

// 导出用户状态以供组件使用
export const useUserStore = () => ({
  state: userState,
  ...userStore
})