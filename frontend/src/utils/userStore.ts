import { reactive } from 'vue'
import { userApi, isSuccess } from './api'

export interface UserInfo {
  id: number
  account: string
  nickname: string
  email: string
  phone: string
  role: string
}

export interface UserState {
  isLoggedIn: boolean
  userInfo: UserInfo | null
}

// 创建响应式的用户状态（仅用于前端显示）
const userState = reactive<UserState>({
  isLoggedIn: false,
  userInfo: null
})

/**
 * 用户状态管理 - 无状态架构
 * 所有数据都从后端获取，前端仅用于显示
 */
export const userStore = {
  // 获取用户状态
  getState: () => userState,

  // 检查是否已登录
  isLoggedIn: () => userState.isLoggedIn,

  // 获取用户信息
  getUserInfo: () => userState.userInfo,

  // 设置用户登录状态（仅用于前端显示更新）
  setLoginState: (userInfo: UserInfo) => {
    userState.isLoggedIn = true
    userState.userInfo = userInfo
  },

  // 清除用户登录状态（仅用于前端显示更新）
  clearLoginState: () => {
    userState.isLoggedIn = false
    userState.userInfo = null
  },

  // 验证Token有效性并更新前端状态
  validateToken: async (): Promise<boolean> => {
    try {
      const response = await userApi.validateToken()
      if (isSuccess(response) && response.data) {
        // Token有效，获取用户信息更新前端状态
        const userInfoResponse = await userApi.getUserInfo()
        if (isSuccess(userInfoResponse)) {
          userState.isLoggedIn = true
          userState.userInfo = userInfoResponse.data
          console.log('Token验证成功，用户已登录:', userInfoResponse.data.account)
          return true
        }
      }
      
      // Token无效或获取用户信息失败，清除前端状态
      userStore.clearLoginState()
      console.log('Token验证失败，用户未登录')
      return false
    } catch (error) {
      console.error('Token验证失败:', error)
      userStore.clearLoginState()
      return false
    }
  },

  // 登出
  logout: async (): Promise<boolean> => {
    try {
      // 调用后端登出接口
      const response = await userApi.logout()
      
      // 清除前端显示状态
      userStore.clearLoginState()
      
      const success = isSuccess(response)
      console.log('用户登出', success ? '成功' : '失败')
      return success
    } catch (error) {
      console.error('登出失败:', error)
      // 即使请求失败，也清除前端状态
      userStore.clearLoginState()
      return false
    }
  },

  // 刷新用户信息
  refreshUserInfo: async (): Promise<boolean> => {
    try {
      const response = await userApi.getUserInfo()
      if (isSuccess(response)) {
        userState.userInfo = response.data
        return true
      }
      return false
    } catch (error) {
      console.error('刷新用户信息失败:', error)
      return false
    }
  },

  // 初始化用户状态（应用启动时调用）
  initialize: async (): Promise<boolean> => {
    try {
      console.log('初始化用户状态...')
      // 使用后端的check_login_status接口检查登录状态
      const response = await userApi.checkLoginStatus()
      
      if (isSuccess(response) && response.data) {
        // 用户已登录，更新前端显示状态
        userState.isLoggedIn = true
        userState.userInfo = response.data
        console.log('用户状态初始化成功，用户已登录:', response.data.account)
        return true
      } else {
        // 用户未登录，清除前端状态
        userStore.clearLoginState()
        console.log('用户状态初始化完成，用户未登录')
        return false
      }
    } catch (error) {
      console.error('用户状态初始化失败:', error)
      userStore.clearLoginState()
      return false
    }
  }
}

// 导出用户状态以供组件使用
export const useUserStore = () => ({
  state: userState,
  ...userStore
})