<template>
  <div v-if="modelValue" class="qrcode-overlay" @click.self="handleClose">
    <div class="qrcode-dialog responsive-dialog" :class="responsiveClasses">
      <div class="dialog-header">
        <h2 class="dialog-title">扫码支付</h2>
        <button class="close-button" @click="handleClose">×</button>
      </div>

      <!-- 支付信息 -->
      <div class="payment-info-section">
        <div class="payment-summary">
          <div class="plan-info">
            <h3>{{ selectedPlan?.title }}</h3>
            <div class="plan-price">
              <span class="price">¥{{ selectedPlan?.price ? (selectedPlan.price / 100).toFixed(2) : '0.00' }}</span>
              <span class="period">/{{ selectedPlan?.period }}</span>
            </div>
          </div>
          <div class="payment-method-info">
            <div class="method-icon">
              <i class="iconfont" :class="paymentMethodIconClass"></i>
            </div>
            <span class="method-name">{{ paymentMethodName }}</span>
          </div>
        </div>
      </div>

      <!-- 二维码显示区域 -->
      <div class="qrcode-section">
        <div v-if="loading" class="qrcode-loading">
          <div class="loading-spinner"></div>
          <p>正在生成支付二维码...</p>
        </div>
        
        <div v-else-if="qrCodeUrl" class="qrcode-container">
          <div class="qrcode-image-wrapper">
            <canvas 
              ref="qrCodeCanvas"
              class="qrcode-image"
              v-show="qrCodeGenerated"
            ></canvas>
            <div v-if="!qrCodeGenerated" class="qrcode-generating">
              <div class="loading-spinner"></div>
              <p>正在生成二维码...</p>
            </div>
          </div>
          <div class="qrcode-tips">
            <div class="tip-item">
              <span class="tip-text">请使用{{ paymentMethodName }}扫描上方二维码完成支付</span>
            </div>
            <div class="tip-item">
              <span class="tip-text">二维码有效期为15分钟，请及时支付</span>
            </div>
          </div>
        </div>

        <div v-else-if="error" class="qrcode-error">
          <div class="error-icon">❌</div>
          <p class="error-message">{{ error }}</p>
          <button class="retry-button" @click="$emit('retry')">重新获取</button>
        </div>
      </div>

      <!-- 操作按钮 -->
      <div class="dialog-actions">
        <button class="cancel-button" @click="handleClose">取消支付</button>
        <button class="refresh-button" @click="handleRefreshStatus" :disabled="refreshing">
          {{ refreshing ? '检查中...' : '我已完成支付' }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, inject } from 'vue'
import { useWindowAdaptive, generateResponsiveClasses } from '../utils/responsive'
import { type PaymentMethodType, type PlanInfo } from './PaymentMethodDialog.vue'
import { vipApi, isSuccess } from '../utils/api'
import QRCode from 'qrcode'

// Props & Emits
interface Props {
  modelValue: boolean
  selectedPlan: PlanInfo | null
  paymentMethod: PaymentMethodType | null
  qrCodeUrl?: string | null
  orderNo?: number | null
  loading?: boolean
  error?: string | null
}

interface Emits {
  (e: 'update:modelValue', value: boolean): void
  (e: 'close'): void
  (e: 'retry'): void
  (e: 'refresh-status'): void
  (e: 'payment-success'): void
}

const props = defineProps<Props>()
const emit = defineEmits<Emits>()

// Composables
const responsive = useWindowAdaptive()
const responsiveClasses = computed(() => generateResponsiveClasses(responsive))
const showMessageBar = inject('showMessageBar') as (message: string, type?: 'info' | 'warning' | 'error') => void

// State
const refreshing = ref(false)
const qrCodeGenerated = ref(false)
const qrCodeCanvas = ref<HTMLCanvasElement | null>(null)

// 支付方式信息
const paymentMethodInfo = {
  wx: {
    name: '微信支付',
    iconClass: 'icon-weixinzhifu'
  },
  ali: {
    name: '支付宝',
    iconClass: 'icon-zhifubaozhifu'
  }
}

// Computed
const paymentMethodName = computed(() => {
  if (!props.paymentMethod) return ''
  return paymentMethodInfo[props.paymentMethod]?.name || ''
})

const paymentMethodIconClass = computed(() => {
  if (!props.paymentMethod) return ''
  return paymentMethodInfo[props.paymentMethod]?.iconClass || ''
})

// Methods
const handleClose = () => {
  emit('update:modelValue', false)
  emit('close')
}

const handleRefreshStatus = async () => {
  if (!props.orderNo) {
    console.error('订单号不存在，无法查询支付结果')
    return
  }

  refreshing.value = true
  try {
    // 调用后端API查询支付结果
    const response = await vipApi.getPayResult({ orderNo: props.orderNo })

    if (isSuccess(response) && response.data) {
      const orderStatus = response.data.orderStatus
      console.log('支付状态查询结果:', orderStatus)

      switch (orderStatus) {
        case 'paid':
          // 支付成功：刷新VIP状态，关闭所有对话框，返回账户页面
          showMessageBar('支付成功！正在更新VIP状态...', 'info')
          emit('refresh-status')
          emit('payment-success')
          break

        case 'unpaid':
          // 未支付：提示用户稍后重试
          showMessageBar('订单尚未支付，请完成支付后重试', 'warning')
          break

        case 'refunding':
          // 退款中：提示用户订单正在退款
          showMessageBar('订单正在退款中，请联系客服', 'info')
          break

        case 'refunded':
          // 已退款：提示用户订单已退款
          showMessageBar('订单已退款，如有疑问请联系客服', 'info')
          break

        default:
          // 未知状态：提示用户稍后重试
          showMessageBar('订单状态异常，请稍后重试', 'error')
          break
      }
    } else {
      console.error('查询支付结果失败:', response.error)
      showMessageBar('查询支付状态失败，请检查网络连接', 'error')
    }
  } catch (error) {
    console.error('查询支付结果出错:', error)
    showMessageBar('网络错误，请检查网络连接后重试', 'error')
  } finally {
    // 延迟重置状态，给用户反馈
    setTimeout(() => {
      refreshing.value = false
    }, 1000)
  }
}

const generateQRCode = async (url: string) => {
  try {
    qrCodeGenerated.value = false
    
    await nextTick() // 确保DOM更新完成
    
    if (!qrCodeCanvas.value) {
      console.error('Canvas元素未找到')
      return
    }

    // 生成二维码的配置
    const options = {
      width: 200,
      height: 200,
      margin: 2,
      color: {
        dark: '#000000',  // 二维码颜色
        light: '#FFFFFF' // 背景颜色
      },
      errorCorrectionLevel: 'M' as const
    }

    // 生成二维码到canvas
    await QRCode.toCanvas(qrCodeCanvas.value, url, options)
    qrCodeGenerated.value = true
    
    console.log('二维码生成成功:', url)
  } catch (error) {
    console.error('生成二维码失败:', error)
    // 可以在这里设置错误状态
  }
}

// Watch for QR code URL changes to generate new QR code
watch(() => props.qrCodeUrl, async (newUrl) => {
  if (newUrl) {
    await generateQRCode(newUrl)
  } else {
    qrCodeGenerated.value = false
  }
}, { immediate: true })
</script>

<style scoped>
.qrcode-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.7);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 2200;
}

.qrcode-dialog {
  background: var(--card-bg, #ffffff);
  border-radius: 12px;
  width: 420px;
  max-width: 95vw;
  max-height: 85vh;
  overflow-y: auto;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  display: flex;
  flex-direction: column;
}

.dialog-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 20px;
  border-bottom: 1px solid var(--border-color, #e2e8f0);
  background: linear-gradient(135deg, var(--header-bg, #2c7a7b), #319795);
  color: white;
  border-radius: 12px 12px 0 0;
  flex-shrink: 0;
}

.dialog-title {
  margin: 0;
  font-size: 1.4rem;
  font-weight: 600;
  color: white;
}

.close-button {
  background: rgba(255, 255, 255, 0.2);
  border: none;
  font-size: 1.8rem;
  cursor: pointer;
  color: white;
  padding: 0;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  transition: all 0.2s ease;
}

.close-button:hover {
  background: rgba(255, 255, 255, 0.3);
  transform: scale(1.1);
}

/* 支付信息区域 */
.payment-info-section {
  padding: 16px 20px;
  border-bottom: 1px solid var(--border-color, #e2e8f0);
  background: var(--bg-secondary, #f8fafc);
}

.payment-summary {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.plan-info h3 {
  margin: 0 0 4px 0;
  font-size: 1.1rem;
  color: var(--text-primary, #333);
}

.plan-price {
  display: flex;
  align-items: baseline;
  gap: 4px;
}

.price {
  font-size: 1.5rem;
  font-weight: 700;
  color: var(--primary-color, #2c7a7b);
}

.period {
  color: var(--text-secondary, #666);
  font-size: 0.9rem;
}

.payment-method-info {
  display: flex;
  align-items: center;
  gap: 8px;
}

.method-icon i {
  font-size: 20px;
  width: 36px;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  color: white;
  line-height: 1;
  text-align: center;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.1);
}

.method-icon i.icon-weixinzhifu {
  background: linear-gradient(135deg, #09B168, #0FA866);
  font-size: 16px;
}

.method-icon i.icon-zhifubaozhifu {
  background: linear-gradient(135deg, #009FE8, #0088CC);
  font-size: 16px;
}

.method-name {
  font-size: 0.9rem;
  color: var(--text-secondary, #666);
  font-weight: 500;
}

/* 二维码区域 */
.qrcode-section {
  flex: 1;
  padding: 24px 20px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 300px;
}

.qrcode-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
}

.loading-spinner {
  width: 40px;
  height: 40px;
  border: 4px solid var(--border-color, #e2e8f0);
  border-top: 4px solid var(--primary-color, #2c7a7b);
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  0% { transform: rotate(0deg); }
  100% { transform: rotate(360deg); }
}

.qrcode-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 20px;
  width: 100%;
}

.qrcode-image-wrapper {
  display: flex;
  justify-content: center;
  align-items: center;
  padding: 16px;
  background: white;
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
}

.qrcode-image {
  max-width: 200px;
  max-height: 200px;
  width: 200px;
  height: 200px;
  display: block;
  border-radius: 4px;
}

.qrcode-generating {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  width: 200px;
  height: 200px;
  justify-content: center;
}

.qrcode-tips {
  display: flex;
  flex-direction: column;
  gap: 8px;
  text-align: center;
}

.tip-item {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  font-size: 0.9rem;
  color: var(--text-secondary, #666);
}

.tip-icon {
  font-size: 1rem;
}

.qrcode-error {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  text-align: center;
}

.error-icon {
  font-size: 3rem;
}

.error-message {
  margin: 0;
  color: var(--text-secondary, #666);
  font-size: 0.95rem;
}

.retry-button {
  padding: 8px 16px;
  border: none;
  border-radius: 6px;
  background: var(--primary-color, #2c7a7b);
  color: white;
  cursor: pointer;
  font-weight: 500;
  transition: all 0.2s ease;
}

.retry-button:hover {
  background: #319795;
  transform: translateY(-1px);
}

/* 操作按钮 */
.dialog-actions {
  display: flex;
  gap: 12px;
  padding: 16px 20px;
  border-top: 1px solid var(--border-color, #e2e8f0);
  background: var(--card-bg, #ffffff);
  border-radius: 0 0 12px 12px;
  flex-shrink: 0;
}

.cancel-button,
.refresh-button {
  flex: 1;
  padding: 10px 16px;
  border: none;
  border-radius: 6px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
  font-size: 0.95rem;
}

.cancel-button {
  background: var(--bg-secondary, #f1f5f9);
  color: var(--text-secondary, #64748b);
  border: 1px solid var(--border-color, #e2e8f0);
}

.cancel-button:hover {
  background: var(--border-color, #e2e8f0);
}

.refresh-button {
  background: linear-gradient(135deg, var(--primary-color, #2c7a7b), #319795);
  color: white;
  box-shadow: 0 4px 12px rgba(44, 122, 123, 0.2);
}

.refresh-button:hover:not(:disabled) {
  background: linear-gradient(135deg, #319795, #2dd4bf);
  box-shadow: 0 6px 20px rgba(44, 122, 123, 0.3);
  transform: translateY(-1px);
}

.refresh-button:disabled {
  opacity: 0.6;
  cursor: not-allowed;
  transform: none;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .qrcode-dialog {
    width: 95vw;
    max-height: 90vh;
    border-radius: 8px;
  }

  .dialog-header {
    padding: 12px 16px;
    border-radius: 8px 8px 0 0;
  }

  .dialog-title {
    font-size: 1.2rem;
  }

  .payment-info-section,
  .qrcode-section,
  .dialog-actions {
    padding: 12px 16px;
  }

  .payment-summary {
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }

  .qrcode-section {
    min-height: 250px;
    padding: 16px;
  }

  .qrcode-image {
    max-width: 160px;
    max-height: 160px;
  }
}

@media (max-width: 480px) {
  .qrcode-dialog {
    width: 98vw;
    max-height: 95vh;
    border-radius: 6px;
  }

  .dialog-header {
    padding: 10px 12px;
    border-radius: 6px 6px 0 0;
  }

  .dialog-title {
    font-size: 1.1rem;
  }

  .payment-info-section,
  .qrcode-section,
  .dialog-actions {
    padding: 10px 12px;
  }

  .qrcode-section {
    min-height: 220px;
    padding: 12px;
  }

  .qrcode-image {
    max-width: 140px;
    max-height: 140px;
  }

  .dialog-actions {
    flex-direction: column;
    gap: 8px;
  }

  .cancel-button,
  .refresh-button {
    width: 100%;
    padding: 12px;
    font-size: 0.9rem;
  }

  .tip-item {
    font-size: 0.8rem;
  }
}

/* 暗色模式支持 */
@media (prefers-color-scheme: dark) {
  .qrcode-dialog {
    --card-bg: #2d2d2d;
    --text-primary: #e6e6e6;
    --text-secondary: #999999;
    --bg-secondary: #3a3a3a;
    --border-color: #3d3d3d;
    --header-bg: #1e3a3a;
  }
  
  .payment-info-section {
    background: var(--bg-secondary, #3a3a3a);
  }
  
  .qrcode-image-wrapper {
    background: #ffffff;
  }
}
</style>