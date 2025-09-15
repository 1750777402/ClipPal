<template>
  <div v-if="modelValue" class="vip-upgrade-overlay" @click.self="handleClose">
    <div class="vip-upgrade-dialog responsive-dialog" :class="responsiveClasses">
      <div class="dialog-header">
        <h2 class="dialog-title">升级VIP会员</h2>
        <button class="close-button" @click="handleClose">×</button>
      </div>
      

      <!-- VIP方案选择 -->
      <div class="plans-section">
        <h3>选择会员方案</h3>
        <div class="plans-grid">
          <div v-for="plan in vipPlans" :key="plan.type" 
               class="plan-card" 
               :class="{ 'recommended': plan.recommended }">
            <div class="plan-badge" v-if="plan.recommended">推荐</div>
            
            <div class="plan-header">
              <h4>{{ plan.title }}</h4>
              <div class="plan-price">
                <span class="price">¥{{ plan.price }}</span>
                <span class="period">/{{ plan.period }}</span>
              </div>
            </div>
            
            <div class="plan-features">
              <div class="feature-item" v-for="feature in plan.features" :key="feature">
                <span class="feature-icon">✓</span>
                <span class="feature-text">{{ feature }}</span>
              </div>
            </div>
            
            <button class="plan-button" @click="handlePurchase(plan)">
              {{ plan.buttonText }}
            </button>
          </div>
        </div>
      </div>

      <!-- 购买引导 -->
      <div class="purchase-guide" v-if="showPurchaseGuide">
        <div class="guide-content">
          <div class="guide-icon">🔄</div>
          <div class="guide-text">
            <p>完成支付后，请点击下方按钮刷新状态</p>
          </div>
          <button class="refresh-btn" @click="handleRefreshStatus" :disabled="vipStore.loading">
            {{ vipStore.loading ? '检查中...' : '刷新VIP状态' }}
          </button>
        </div>
      </div>
    </div>
  </div>

  <!-- 支付方式选择弹框 -->
  <PaymentMethodDialog
    v-model="showPaymentDialog"
    :selected-plan="selectedPlan"
    @confirm="handlePaymentConfirm"
    @cancel="handlePaymentCancel"
  />

  <!-- 二维码支付弹框 -->
  <QRCodeDialog
    v-model="showQRCodeDialog"
    :selected-plan="selectedPlan"
    :payment-method="selectedPaymentMethod"
    :qr-code-url="paymentUrl"
    :loading="qrCodeLoading"
    :error="qrCodeError"
    @close="handleQRCodeClose"
    @retry="handleRetryQRCode"
    @refresh-status="handleRefreshStatus"
  />
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useVipStore } from '../utils/vipStore'
import { useWindowAdaptive, generateResponsiveClasses } from '../utils/responsive'
import { vipApi, isSuccess } from '../utils/api'
import PaymentMethodDialog, { type PaymentMethodType, type PlanInfo } from './PaymentMethodDialog.vue'
import QRCodeDialog from './QRCodeDialog.vue'

// Props & Emits
interface Props {
  modelValue: boolean
}

interface Emits {
  (e: 'update:modelValue', value: boolean): void
}

defineProps<Props>()
const emit = defineEmits<Emits>()

// Composables
const responsive = useWindowAdaptive()
const responsiveClasses = computed(() => generateResponsiveClasses(responsive))
const vipStore = useVipStore()

// State
const showPurchaseGuide = ref(false)
const showPaymentDialog = ref(false)
const showQRCodeDialog = ref(false)
const selectedPlan = ref<PlanInfo | null>(null)
const selectedPaymentMethod = ref<PaymentMethodType | null>(null)
const paymentUrl = ref<string | null>(null)
const qrCodeLoading = ref(false)
const qrCodeError = ref<string | null>(null)

// VIP方案配置（基于服务器配置）
const vipPlans = computed(() => {
  try {
    const benefits = vipStore.getVipBenefits?.value || {}
    
    return [
      {
        type: 'Monthly',
        title: '月度会员',
        price: 6,
        period: '月',
        features: benefits.Monthly?.features || ['300条记录存储', '3MB文件上传', '多设备同步'],
        buttonText: '开通月度会员',
        recommended: false
      },
      {
        type: 'Quarterly', 
        title: '季度会员',
        price: 15,
        period: '3个月',
        features: benefits.Quarterly?.features || ['500条记录存储', '4MB文件上传', '多设备同步', '季度优惠价'],
        buttonText: '开通季度会员',
        recommended: true
      },
      {
        type: 'Yearly',
        title: '年度会员',
        price: 60,
        period: '12个月',
        features: benefits.Yearly?.features || ['1000条记录存储', '5MB文件上传', '多设备同步', '年度超值价'],
        buttonText: '开通年度会员',
        recommended: false
      }
    ]
  } catch (error) {
    console.error('生成VIP方案配置失败:', error)
    return [
      {
        type: 'Monthly',
        title: '月度会员',
        price: 6,
        period: '月',
        features: ['300条记录存储', '3MB文件上传', '多设备同步'],
        buttonText: '开通月度会员',
        recommended: false
      },
      {
        type: 'Quarterly', 
        title: '季度会员',
        price: 15,
        period: '3个月',
        features: ['400条记录存储', '4MB文件上传', '多设备同步', '季度优惠价'],
        buttonText: '开通季度会员',
        recommended: true
      },
      {
        type: 'Yearly',
        title: '年度会员',
        price: 60,
        period: '12个月',
        features: ['1000条记录存储', '5MB文件上传', '多设备同步', '年度超值价'],
        buttonText: '开通年度会员',
        recommended: false
      }
    ]
  }
})

// Methods
const handleClose = () => {
  emit('update:modelValue', false)
  showPurchaseGuide.value = false
  showPaymentDialog.value = false
  showQRCodeDialog.value = false
  selectedPlan.value = null
  selectedPaymentMethod.value = null
  paymentUrl.value = null
  qrCodeError.value = null
}

const handlePurchase = async (plan: any) => {
  try {
    selectedPlan.value = {
      type: plan.type,
      title: plan.title,
      price: plan.price,
      period: plan.period
    }
    showPaymentDialog.value = true
  } catch (error) {
    console.error('选择方案失败:', error)
  }
}

const handleRefreshStatus = async () => {
  try {
    const updated = await vipStore.refreshStatus()
    if (updated) {
      showPurchaseGuide.value = false
    }
  } catch (error) {
    console.error('刷新状态失败:', error)
  }
}

const handlePaymentConfirm = async (data: { planType: string; paymentMethod: PaymentMethodType }) => {
  try {
    console.log('选择的支付方式:', data)
    
    // 保存选择的支付方式
    selectedPaymentMethod.value = data.paymentMethod
    
    // 关闭支付方式选择弹框，显示二维码弹框
    showPaymentDialog.value = false
    showQRCodeDialog.value = true
    qrCodeLoading.value = true
    qrCodeError.value = null
    
    // 调用后端API获取支付二维码URL
    const response = await vipApi.getPayUrl({
      vipType: data.planType,
      payType: data.paymentMethod
    })
    
    qrCodeLoading.value = false
    
    if (isSuccess(response) && response.data) {
      // 获取支付URL成功
      paymentUrl.value = response.data
      console.log('获取支付URL成功:', response.data)
    } else {
      // 获取支付URL失败
      qrCodeError.value = response.error || '获取支付链接失败，请重试'
      console.error('获取支付URL失败:', response.error)
    }
  } catch (error) {
    qrCodeLoading.value = false
    qrCodeError.value = '网络错误，请检查网络连接后重试'
    console.error('处理支付失败:', error)
  }
}

const handlePaymentCancel = () => {
  showPaymentDialog.value = false
  selectedPlan.value = null
}

const handleQRCodeClose = () => {
  showQRCodeDialog.value = false
  selectedPaymentMethod.value = null
  paymentUrl.value = null
  qrCodeError.value = null
}

const handleRetryQRCode = async () => {
  if (!selectedPlan.value || !selectedPaymentMethod.value) return
  
  // 重新获取二维码
  await handlePaymentConfirm({
    planType: selectedPlan.value.type,
    paymentMethod: selectedPaymentMethod.value
  })
}
</script>

<style scoped>
.vip-upgrade-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.7);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 2000;
}

.vip-upgrade-dialog {
  background: var(--card-bg, #ffffff);
  border-radius: 12px;
  width: 680px;
  max-width: 95vw;
  max-height: 75vh;
  min-height: 300px;
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
  font-size: 1.5rem;
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


/* VIP方案 */
.plans-section {
  padding: 16px 20px;
  background: var(--card-bg, #ffffff);
  flex: 1;
  overflow-y: auto;
}

/* 当没有购买引导时，为最后一个区域添加底部圆角 */
.plans-section:last-child {
  border-radius: 0 0 12px 12px;
}

.plans-section h3 {
  margin: 0 0 16px 0;
  font-size: 1.2rem;
  color: var(--text-primary, #333);
}

.plans-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 10px;
}

.plan-card {
  position: relative;
  border: 2px solid var(--border-color, #e2e8f0);
  border-radius: 8px;
  padding: 14px 10px;
  text-align: center;
  transition: all 0.3s ease;
  background: var(--card-bg, #ffffff);
  min-height: 230px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
}

.plan-card:hover {
  border-color: var(--primary-color, #2c7a7b);
  transform: translateY(-2px);
  box-shadow: 0 8px 25px rgba(0, 0, 0, 0.1);
}

.plan-card.recommended {
  border-color: var(--primary-color, #2c7a7b);
  box-shadow: 0 4px 12px rgba(44, 122, 123, 0.2);
}

.plan-badge {
  position: absolute;
  top: -10px;
  left: 50%;
  transform: translateX(-50%);
  background: var(--primary-color, #2c7a7b);
  color: white;
  padding: 4px 12px;
  border-radius: 12px;
  font-size: 0.8rem;
  font-weight: 600;
}

.plan-header h4 {
  margin: 0 0 6px 0;
  font-size: 1rem;
  color: var(--text-primary, #333);
}

.plan-price {
  margin-bottom: 12px;
}

.price {
  font-size: 1.8rem;
  font-weight: 700;
  color: var(--primary-color, #2c7a7b);
}

.period {
  color: var(--text-secondary, #666);
  font-size: 0.9rem;
}

.plan-features {
  margin-bottom: 16px;
  text-align: left;
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 4px 8px;
}

.feature-item {
  display: flex;
  align-items: center;
}

.feature-icon {
  color: #10b981;
  margin-right: 8px;
  font-weight: 600;
}

.feature-text {
  font-size: 1rem;
  color: var(--text-secondary, #666);
}

.plan-button {
  width: 100%;
  padding: 10px 8px;
  border: none;
  border-radius: 6px;
  background: linear-gradient(135deg, var(--header-bg, #2c7a7b), #319795);
  color: white;
  font-weight: 600;
  font-size: 0.9rem;
  cursor: pointer;
  transition: all 0.3s ease;
  box-shadow: 0 4px 12px rgba(44, 122, 123, 0.2);
}

.plan-button:hover {
  background: linear-gradient(135deg, #319795, #2dd4bf);
  box-shadow: 0 6px 20px rgba(44, 122, 123, 0.3);
  transform: translateY(-1px);
}


/* 购买引导 */
.purchase-guide {
  padding: 16px 20px;
  border-top: 1px solid var(--border-color, #e2e8f0);
  background: var(--bg-secondary, #f8fafc);
  border-radius: 0 0 12px 12px;
  flex-shrink: 0;
}

.guide-content {
  display: flex;
  align-items: center;
  gap: 16px;
}

.guide-icon {
  font-size: 2rem;
}

.guide-text {
  flex: 1;
}

.refresh-btn {
  padding: 8px 16px;
  border: none;
  border-radius: 6px;
  background: linear-gradient(135deg, var(--header-bg, #2c7a7b), #319795);
  color: white;
  cursor: pointer;
  font-weight: 600;
  transition: all 0.2s ease;
  box-shadow: 0 2px 8px rgba(44, 122, 123, 0.2);
}

.refresh-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.refresh-btn:hover:not(:disabled) {
  background: linear-gradient(135deg, #319795, #2dd4bf);
  box-shadow: 0 4px 12px rgba(44, 122, 123, 0.3);
  transform: translateY(-1px);
}


/* 响应式设计 */
@media (max-width: 768px) {
  .vip-upgrade-dialog {
    width: 95vw;
    max-height: 85vh;
    border-radius: 8px;
  }

  .dialog-header {
    padding: 12px 16px;
    border-radius: 8px 8px 0 0;
  }

  .dialog-title {
    font-size: 1.3rem;
  }


  .plans-section {
    padding: 12px 16px;
  }

  .plans-section h3 {
    font-size: 1.1rem;
    margin-bottom: 12px;
  }

  .plans-grid {
    grid-template-columns: 1fr;
    gap: 12px;
  }

  .plan-card {
    min-height: auto;
    padding: 16px 12px;
  }

  .plan-header h4 {
    font-size: 1.1rem;
    margin-bottom: 8px;
  }

  .price {
    font-size: 2rem;
  }

  .plan-features {
    grid-template-columns: 1fr;
    gap: 6px 0;
  }

  .feature-text {
    font-size: 0.9rem;
  }

  .plan-button {
    padding: 12px;
    font-size: 1rem;
  }

  .purchase-guide {
    padding: 12px 16px;
    border-radius: 0 0 8px 8px;
  }

  .guide-content {
    flex-direction: column;
    text-align: center;
    gap: 12px;
  }

  .refresh-btn {
    width: 100%;
  }
}

@media (max-width: 480px) {
  .vip-upgrade-dialog {
    width: 98vw;
    max-height: 90vh;
    min-height: 280px;
    border-radius: 6px;
  }

  .dialog-header {
    padding: 8px 12px;
    border-radius: 6px 6px 0 0;
    flex-shrink: 0;
  }

  .dialog-title {
    font-size: 1.2rem;
  }

  .plans-section,
  .purchase-guide {
    padding: 8px 12px;
  }

  .plans-section h3 {
    font-size: 1rem;
    margin-bottom: 8px;
  }

  .plan-card {
    padding: 10px 6px;
    min-height: 220px;
  }

  .price {
    font-size: 1.6rem;
  }

  .plan-features {
    grid-template-columns: 1fr;
    gap: 4px 0;
  }

  .feature-text {
    font-size: 0.85rem;
  }

  .plan-button {
    padding: 10px 6px;
    font-size: 0.9rem;
  }
}

/* 极限小屏幕适配 (360px以下) */
@media (max-width: 360px) {
  .vip-upgrade-dialog {
    width: 100vw;
    max-height: 95vh;
    min-height: 250px;
    border-radius: 0;
    margin: 0;
  }

  .dialog-header {
    padding: 6px 10px;
    border-radius: 0;
  }

  .dialog-title {
    font-size: 1.1rem;
  }

  .current-status,
  .plans-section,
  .purchase-guide {
    padding: 6px 10px;
  }

  .plans-section h3 {
    font-size: 0.95rem;
    margin-bottom: 6px;
  }

  .plan-card {
    padding: 8px 4px;
    min-height: 200px;
  }

  .plan-header h4 {
    font-size: 0.9rem;
    margin-bottom: 4px;
  }

  .price {
    font-size: 1.4rem;
  }

  .period {
    font-size: 0.8rem;
  }

  .plan-price {
    margin-bottom: 8px;
  }

  .plan-features {
    margin-bottom: 12px;
    grid-template-columns: 1fr;
    gap: 3px 0;
  }

  .feature-text {
    font-size: 0.8rem;
  }

  .plan-button {
    padding: 8px 4px;
    font-size: 0.85rem;
  }

  .status-card {
    padding: 12px;
  }

  .status-icon {
    font-size: 1.5rem;
    margin-right: 12px;
  }

  .status-title {
    font-size: 1rem;
  }

  .status-detail {
    font-size: 0.8rem;
  }
}

/* 超极限屏幕适配 (320px以下) */
@media (max-width: 320px) {
  .vip-upgrade-dialog {
    width: 100vw;
    max-height: 100vh;
    min-height: 200px;
    border-radius: 0;
  }

  .dialog-header {
    padding: 4px 8px;
  }

  .dialog-title {
    font-size: 1rem;
  }

  .close-button {
    width: 28px;
    height: 28px;
    font-size: 1.4rem;
  }

  .plans-section,
  .purchase-guide {
    padding: 4px 8px;
  }

  .plan-card {
    padding: 6px 3px;
    min-height: 180px;
  }

  .price {
    font-size: 1.2rem;
  }

  .plan-features {
    grid-template-columns: 1fr;
    gap: 2px 0;
  }

  .feature-text {
    font-size: 0.75rem;
  }

  .plan-button {
    padding: 6px 3px;
    font-size: 0.8rem;
  }
}

/* 暗色模式支持 */
@media (prefers-color-scheme: dark) {
  .vip-upgrade-dialog {
    --card-bg: #2d2d2d;
    --text-primary: #e6e6e6;
    --text-secondary: #999999;
    --bg-secondary: #3a3a3a;
    --border-color: #3d3d3d;
    --header-bg: #1e3a3a;
  }
  
  .current-status,
  .plans-section {
    background: var(--card-bg, #2d2d2d);
  }
  
  .purchase-guide {
    background: var(--bg-secondary, #3a3a3a);
  }
}
</style>