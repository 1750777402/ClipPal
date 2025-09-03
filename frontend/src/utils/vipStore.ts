import { reactive, computed } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { apiInvoke, isSuccess } from './api'

export interface VipInfo {
  vip_flag: boolean
  vip_type: 'Free' | 'Monthly' | 'Quarterly' | 'Yearly'
  expire_time?: number
  max_records: number
  max_sync_records?: number // 保留字段但标记为可选，后续可以完全移除
  max_file_size: number // 服务端返回的KB单位
  features: string[]
}

export interface VipLimits {
  isVip: boolean
  maxRecords: number
  maxFileSize: number // 转换为字节单位
  canCloudSync: boolean
}

export interface ServerConfigResponse {
  maxFileSize: number
  recordLimit: number
  syncCheckInterval: number
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
  serverConfig: null as Record<string, ServerConfigResponse> | null,
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
  get serverConfig() { return vipState.serverConfig },
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

  // vip是否生效中显示
  vipFlagDisplay: computed(() => {
    return vipState.vipInfo?.vip_flag ? "生效中" : "已失效"
  }),

  // 获取当前用户类型的服务器配置
  currentServerConfig: computed(() => {
    if (!vipState.serverConfig) return null

    // 映射前端VIP类型到服务端key
    const getServerKey = (vipType: string) => {
      switch (vipType) {
        case 'Free': return 'Free'
        case 'Monthly': return 'Monthly'
        case 'Quarterly': return 'Quarterly'
        case 'Yearly': return 'Yearly'
        default: return 'Free'
      }
    }

    const userType = vipState.vipInfo?.vip_flag ? vipState.vipInfo.vip_type : 'Free'
    const serverKey = getServerKey(userType)
    return vipState.serverConfig[serverKey] || null
  }),

  // 获取VIP会员权益配置（基于服务器配置）
  getVipBenefits: computed(() => {
    if (!vipState.serverConfig) return {}

    return {
      Free: {
        name: '免费用户',
        features: [
          `${vipState.serverConfig.Free?.recordLimit || 300}条本地记录`,
          '基础功能使用',
          '有限制的文件上传'
        ]
      },
      Monthly: {
        name: '月度会员',
        features: [
          `${vipState.serverConfig.Monthly?.recordLimit || 300}条本地记录`,
          `${((vipState.serverConfig.Monthly?.maxFileSize || 3072) / 1024).toFixed(0)}MB文件上传`,
          '多设备同步',
          '高级功能解锁'
        ]
      },
      Quarterly: {
        name: '季度会员',
        features: [
          `${vipState.serverConfig.Quarterly?.recordLimit || 500}条本地记录`,
          `${((vipState.serverConfig.Quarterly?.maxFileSize || 4096) / 1024).toFixed(0)}MB文件上传`,
          '多设备同步',
          '高级功能解锁',
          '季度优惠价'
        ]
      },
      Yearly: {
        name: '年度会员',
        features: [
          `${vipState.serverConfig.Yearly?.recordLimit || 1000}条本地记录`,
          `${((vipState.serverConfig.Yearly?.maxFileSize || 5120) / 1024).toFixed(0)}MB文件上传`,
          '多设备同步',
          '高级功能解锁',
          '年度超值价'
        ]
      }
    }
  }),

  // 初始化VIP状态
  async initialize(): Promise<boolean> {
    if (vipState.initialized) return true

    vipState.loading = true
    try {
      const success = await Promise.all([
        this.loadVipStatus(),
        this.loadVipLimits(),
        this.loadServerConfig()
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

  // 加载服务器配置信息
  async loadServerConfig(): Promise<boolean> {
    try {
      const response = await apiInvoke<Record<string, ServerConfigResponse>>('get_server_config')
      if (isSuccess(response)) {
        vipState.serverConfig = response.data || null
        return true
      }
      return false
    } catch (error) {
      console.error('获取服务器配置失败:', error)
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

  // 刷新VIP状态 - 只调用一次refresh_vip_status，后端会更新所有相关数据
  async refreshStatus(): Promise<boolean> {
    vipState.loading = true
    try {
      // refresh_vip_status 会刷新VIP状态、限制和服务器配置，只需调用一次
      const response = await apiInvoke<boolean>('refresh_vip_status')

      if (isSuccess(response)) {
        // 刷新成功后，从本地存储加载最新数据（避免重复网络请求）
        const [statusLoaded, limitsLoaded, configLoaded] = await Promise.all([
          this.loadVipStatus(),
          this.loadVipLimits(),
          this.loadServerConfig()
        ])
        console.log('VIP状态已刷新')
        return statusLoaded && limitsLoaded && configLoaded
      } else {
        console.log('VIP状态刷新失败，使用本地缓存')
        return false
      }
    } catch (error) {
      console.warn('VIP状态刷新失败:', error)
      return false
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