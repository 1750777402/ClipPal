import { reactive, computed } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { apiInvoke, isSuccess } from './api'

export interface VipInfo {
  vip_flag: boolean
  vip_type: 'Free' | 'Monthly' | 'Quarterly' | 'Yearly'
  expire_time?: number
  max_records: number
  max_sync_records: number
  max_file_size: number
  features: string[]
}

export interface VipLimits {
  isVip: boolean
  maxRecords: number
  maxFileSize: number
  maxSyncRecords: number
  canCloudSync: boolean
}

export interface VipStatusChangedPayload {
  is_vip: boolean
  vip_type?: 'Free' | 'Monthly' | 'Quarterly' | 'Yearly'
  expire_time?: number
  max_records: number
}

// 创建响应式的VIP状态
const vipState = reactive({
  vipInfo: null as VipInfo | null,
  limits: null as VipLimits | null,
  loading: false,
  initialized: false
})

/**
 * VIP状态管理 - 无状态架构
 * 所有数据都从后端获取，前端仅用于显示
 */
export const vipStore = {
  // 状态访问器
  get vipInfo() { return vipState.vipInfo },
  get limits() { return vipState.limits },
  get loading() { return vipState.loading },
  get initialized() { return vipState.initialized },

  // 计算属性
  isVip: computed(() => vipState.vipInfo?.vip_flag ?? false),
  canCloudSync: computed(() => vipState.limits?.canCloudSync ?? false),
  maxRecordsLimit: computed(() => vipState.limits?.maxRecords ?? 500),

  // VIP类型显示名称
  vipTypeDisplay: computed(() => {
    switch (vipState.vipInfo?.vip_type) {
      case 'Monthly': return '月度会员'
      case 'Quarterly': return '季度会员'
      case 'Yearly': return '年度会员'
      default: return '免费用户'
    }
  }),

  // 过期时间显示
  expireTimeDisplay: computed(() => {
    if (!vipState.vipInfo?.expire_time) return null
    return new Date(vipState.vipInfo.expire_time * 1000).toLocaleDateString('zh-CN')
  }),

  // 初始化VIP状态
  async initialize(): Promise<boolean> {
    if (vipState.initialized) return true

    vipState.loading = true
    try {
      const success = await Promise.all([
        this.loadVipStatus(),
        this.loadVipLimits()
      ])

      // 监听VIP状态变更事件
      await this.setupEventListeners()

      vipState.initialized = true
      return success.every(Boolean)

    } catch (error) {
      console.error('初始化VIP状态失败:', error)
      return false
    } finally {
      vipState.loading = false
    }
  },

  // 加载VIP状态
  async loadVipStatus(): Promise<boolean> {
    try {
      const response = await apiInvoke<VipInfo>('get_vip_status')
      if (isSuccess(response)) {
        vipState.vipInfo = response.data || null
        return true
      }
      return false
    } catch (error) {
      console.error('获取VIP状态失败:', error)
      return false
    }
  },

  // 加载VIP限制信息
  async loadVipLimits(): Promise<boolean> {
    try {
      const response = await apiInvoke<VipLimits>('get_vip_limits')
      if (isSuccess(response)) {
        vipState.limits = response.data || null
        return true
      }
      return false
    } catch (error) {
      console.error('获取VIP限制失败:', error)
      return false
    }
  },

  // 检查云同步权限
  async checkCloudSyncPermission(): Promise<{ allowed: boolean, message: string }> {
    try {
      const response = await apiInvoke<[boolean, string]>('check_vip_permission')
      if (isSuccess(response)) {
        return {
          allowed: response.data![0],
          message: response.data![1]
        }
      }
    } catch (error) {
      console.error('检查云同步权限失败:', error)
    }
    return { allowed: false, message: '权限检查失败' }
  },

  // 打开VIP购买页面
  async openPurchasePage(): Promise<boolean> {
    try {
      const response = await apiInvoke('open_vip_purchase_page')
      return isSuccess(response)
    } catch (error) {
      console.error('打开购买页面失败:', error)
      throw error
    }
  },

  // 刷新VIP状态 - 服务端优先，失败时展示本地缓存
  async refreshStatus(): Promise<boolean> {
    vipState.loading = true
    try {
      // 尝试从服务端刷新
      const response = await apiInvoke<boolean>('refresh_vip_status')

      if (isSuccess(response) && response.data) {
        // 服务端更新成功，加载最新的本地数据
        const [statusLoaded, limitsLoaded] = await Promise.all([
          this.loadVipStatus(),
          this.loadVipLimits()
        ])
        console.log('VIP状态已从服务器更新')
        return statusLoaded && limitsLoaded
      } else {
        // 服务端更新失败，仅从本地缓存加载
        console.log('服务端更新失败，使用本地缓存的VIP状态')
        const statusLoaded = await this.loadVipStatus()
        // 如果有本地VIP数据，也加载limits；否则跳过避免触发服务端调用
        if (vipState.vipInfo) {
          const limitsLoaded = await this.loadVipLimits()
          return statusLoaded && limitsLoaded
        }
        return statusLoaded
      }
    } catch (error) {
      console.warn('服务端刷新失败，尝试加载本地缓存:', error)

      // 服务端失败时，尝试加载本地缓存
      const statusLoaded = await this.loadVipStatus()
      // 只有在有本地数据时才加载limits
      if (vipState.vipInfo) {
        const limitsLoaded = await this.loadVipLimits()
        return statusLoaded && limitsLoaded
      }
      return statusLoaded
    } finally {
      vipState.loading = false
    }
  },

  // 模拟VIP升级（测试用）
  async simulateUpgrade(vipType: 'Monthly' | 'Quarterly' | 'Yearly', days: number): Promise<boolean> {
    try {
      const response = await apiInvoke('simulate_vip_upgrade', { vip_type: vipType, days })
      return isSuccess(response)
    } catch (error) {
      console.error('模拟VIP升级失败:', error)
      return false
    }
  },

  // 设置事件监听器
  async setupEventListeners(): Promise<void> {
    try {
      // 监听VIP状态变更事件
      await listen('vip-status-changed', (event: any) => {
        console.log('VIP状态已变更:', event.payload)
        this.loadVipStatus()
        this.loadVipLimits()
      })
    } catch (error) {
      console.error('设置VIP事件监听器失败:', error)
    }
  },

  // 清除VIP状态（用于登出时）
  clearVipState(): void {
    vipState.vipInfo = null
    vipState.limits = null
    vipState.initialized = false
    console.log('VIP状态已清除')
  },

  // 获取VIP功能权限
  hasFeature(feature: string): boolean {
    return vipState.vipInfo?.features.includes(feature) ?? false
  },

  // 检查是否接近过期（7天内）
  isExpiringSoon: computed(() => {
    if (!vipState.vipInfo?.expire_time) return false
    const now = Date.now() / 1000
    const expireTime = vipState.vipInfo.expire_time
    const sevenDaysInSeconds = 7 * 24 * 3600
    return (expireTime - now) <= sevenDaysInSeconds && (expireTime - now) > 0
  }),

  // 获取剩余天数
  remainingDays: computed(() => {
    if (!vipState.vipInfo?.expire_time) return 0
    const now = Date.now() / 1000
    const remaining = vipState.vipInfo.expire_time - now
    return Math.max(0, Math.ceil(remaining / (24 * 3600)))
  }),

  // 检查是否已过期（基于剩余天数，更可靠）
  isExpired: computed(() => {
    console.log("isExpired computed被调用了", vipState.vipInfo)
    if (!vipState.vipInfo?.expire_time) {
      console.log("没有expire_time，返回false")
      return false
    }
    // 直接使用剩余天数判断，避免时间戳精度问题
    const now = Date.now() / 1000
    const remaining = vipState.vipInfo.expire_time - now
    const days = Math.max(0, Math.ceil(remaining / (24 * 3600)))
    console.log("剩余天数:", days, "是否过期:", days <= 0)
    return days <= 0
  }),
}

// 导出VIP状态以供组件使用
export const useVipStore = () => ({
  state: vipState,
  ...vipStore
})