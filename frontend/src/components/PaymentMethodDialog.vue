<template>
  <div v-if="modelValue" class="payment-method-overlay" @click.self="handleCancel">
    <div class="payment-method-dialog responsive-dialog" :class="responsiveClasses">
      <div class="dialog-header">
        <h2 class="dialog-title">选择支付方式</h2>
        <button class="close-button" @click="handleCancel">×</button>
      </div>

      <!-- 选择的会员套餐信息 -->
      <div class="selected-plan-info">
        <div class="plan-summary">
          <h3>{{ selectedPlan?.title }}</h3>
          <div class="plan-price">
            <span class="price">¥{{ selectedPlan?.price }}</span>
            <span class="period">/{{ selectedPlan?.period }}</span>
          </div>
        </div>
      </div>

      <!-- 支付方式选择 -->
      <div class="payment-methods-section">
        <h3>支付方式</h3>
        <div class="payment-methods-grid">
          <div 
            v-for="method in paymentMethods" 
            :key="method.type"
            class="payment-method-card" 
            :class="{ 'selected': selectedPaymentMethod === method.type }"
            @click="selectPaymentMethod(method.type)"
          >
            <div class="payment-icon">
              <i class="iconfont" :class="method.iconClass"></i>
            </div>
            <div class="payment-info">
              <h4>{{ method.name }}</h4>
              <p>{{ method.description }}</p>
            </div>
            <div class="payment-check">
              <span v-if="selectedPaymentMethod === method.type" class="check-icon">✓</span>
            </div>
          </div>
        </div>
      </div>

      <!-- 操作按钮 -->
      <div class="dialog-actions">
        <button class="cancel-button" @click="handleCancel">取消</button>
        <button 
          class="confirm-button" 
          :disabled="!selectedPaymentMethod || loading"
          @click="handleConfirm"
        >
          {{ loading ? '处理中...' : '确认支付' }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useWindowAdaptive, generateResponsiveClasses } from '../utils/responsive'

// 支付方式类型
export type PaymentMethodType = 'wechat' | 'alipay'

// 会员套餐信息接口
export interface PlanInfo {
  type: string
  title: string
  price: number
  period: string
}

// Props & Emits
interface Props {
  modelValue: boolean
  selectedPlan: PlanInfo | null
}

interface Emits {
  (e: 'update:modelValue', value: boolean): void
  (e: 'confirm', data: { planType: string; paymentMethod: PaymentMethodType }): void
  (e: 'cancel'): void
}

const props = defineProps<Props>()
const emit = defineEmits<Emits>()

// Composables
const responsive = useWindowAdaptive()
const responsiveClasses = computed(() => generateResponsiveClasses(responsive))

// State
const selectedPaymentMethod = ref<PaymentMethodType | null>(null)
const loading = ref(false)

// 支付方式配置
const paymentMethods = [
  {
    type: 'wechat' as PaymentMethodType,
    name: '微信支付',
    description: '使用微信扫码支付',
    iconClass: 'icon-weixinzhifu'
  },
  {
    type: 'alipay' as PaymentMethodType,
    name: '支付宝',
    description: '使用支付宝扫码支付',
    iconClass: 'icon-zhifubaozhifu'
  }
]

// Methods
const selectPaymentMethod = (method: PaymentMethodType) => {
  selectedPaymentMethod.value = method
}

const handleCancel = () => {
  emit('update:modelValue', false)
  emit('cancel')
  selectedPaymentMethod.value = null
}

const handleConfirm = async () => {
  if (!selectedPaymentMethod.value || !props.selectedPlan) return
  
  loading.value = true
  try {
    emit('confirm', {
      planType: props.selectedPlan.type,
      paymentMethod: selectedPaymentMethod.value
    })
  } finally {
    loading.value = false
  }
}
</script>

<style scoped>
.payment-method-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.7);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 2100;
}

.payment-method-dialog {
  background: var(--card-bg, #ffffff);
  border-radius: 12px;
  width: 480px;
  max-width: 95vw;
  max-height: 75vh;
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

/* 选择的套餐信息 */
.selected-plan-info {
  padding: 16px 20px;
  border-bottom: 1px solid var(--border-color, #e2e8f0);
  background: var(--bg-secondary, #f8fafc);
}

.plan-summary {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.plan-summary h3 {
  margin: 0;
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

/* 支付方式选择 */
.payment-methods-section {
  padding: 20px;
  flex: 1;
}

.payment-methods-section h3 {
  margin: 0 0 16px 0;
  font-size: 1.1rem;
  color: var(--text-primary, #333);
}

.payment-methods-grid {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.payment-method-card {
  display: flex;
  align-items: center;
  padding: 16px;
  border: 2px solid var(--border-color, #e2e8f0);
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s ease;
  background: var(--card-bg, #ffffff);
}

.payment-method-card:hover {
  border-color: var(--primary-color, #2c7a7b);
  box-shadow: 0 4px 12px rgba(44, 122, 123, 0.1);
}

.payment-method-card.selected {
  border-color: var(--primary-color, #2c7a7b);
  background: rgba(44, 122, 123, 0.05);
  box-shadow: 0 4px 12px rgba(44, 122, 123, 0.2);
}

.payment-icon {
  margin-right: 16px;
  flex-shrink: 0;
}

.payment-icon i {
  font-size: 28px;
  width: 48px;
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  color: white;
  line-height: 1;
  text-align: center;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
  transition: all 0.2s ease;
}

.payment-icon i.icon-weixinzhifu {
  background: linear-gradient(135deg, #09B168, #0FA866);
  font-size: 24px;
}

.payment-icon i.icon-zhifubaozhifu {
  background: linear-gradient(135deg, #009FE8, #0088CC);
  font-size: 24px;
}

.payment-method-card:hover .payment-icon i {
  transform: scale(1.05);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
}

.payment-info {
  flex: 1;
}

.payment-info h4 {
  margin: 0 0 4px 0;
  font-size: 1rem;
  font-weight: 600;
  color: var(--text-primary, #333);
}

.payment-info p {
  margin: 0;
  font-size: 0.9rem;
  color: var(--text-secondary, #666);
}

.payment-check {
  margin-left: 16px;
  flex-shrink: 0;
}

.check-icon {
  display: inline-flex;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: var(--primary-color, #2c7a7b);
  color: white;
  align-items: center;
  justify-content: center;
  font-size: 0.8rem;
  font-weight: 600;
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
.confirm-button {
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

.confirm-button {
  background: linear-gradient(135deg, var(--primary-color, #2c7a7b), #319795);
  color: white;
  box-shadow: 0 4px 12px rgba(44, 122, 123, 0.2);
}

.confirm-button:hover:not(:disabled) {
  background: linear-gradient(135deg, #319795, #2dd4bf);
  box-shadow: 0 6px 20px rgba(44, 122, 123, 0.3);
  transform: translateY(-1px);
}

.confirm-button:disabled {
  opacity: 0.6;
  cursor: not-allowed;
  transform: none;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .payment-method-dialog {
    width: 95vw;
    max-height: 85vh;
    border-radius: 8px;
  }

  .dialog-header {
    padding: 12px 16px;
    border-radius: 8px 8px 0 0;
  }

  .dialog-title {
    font-size: 1.2rem;
  }

  .selected-plan-info,
  .payment-methods-section,
  .dialog-actions {
    padding: 12px 16px;
  }

  .payment-method-card {
    padding: 12px;
  }

  .payment-icon {
    margin-right: 12px;
  }

  .payment-icon i {
    font-size: 24px;
    width: 42px;
    height: 42px;
  }
  
  .payment-icon i.icon-weixinzhifu,
  .payment-icon i.icon-zhifubaozhifu {
    font-size: 20px;
  }

  .payment-info h4 {
    font-size: 0.95rem;
  }

  .payment-info p {
    font-size: 0.85rem;
  }
}

@media (max-width: 480px) {
  .payment-method-dialog {
    width: 98vw;
    max-height: 90vh;
    border-radius: 6px;
  }

  .dialog-header {
    padding: 10px 12px;
    border-radius: 6px 6px 0 0;
  }

  .dialog-title {
    font-size: 1.1rem;
  }

  .selected-plan-info,
  .payment-methods-section,
  .dialog-actions {
    padding: 10px 12px;
  }

  .plan-summary {
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }

  .payment-method-card {
    padding: 10px;
  }

  .payment-icon {
    margin-right: 10px;
  }

  .payment-icon i {
    font-size: 20px;
    width: 38px;
    height: 38px;
  }
  
  .payment-icon i.icon-weixinzhifu,
  .payment-icon i.icon-zhifubaozhifu {
    font-size: 16px;
  }

  .dialog-actions {
    flex-direction: column;
    gap: 8px;
  }

  .cancel-button,
  .confirm-button {
    width: 100%;
    padding: 12px;
    font-size: 0.9rem;
  }
}

/* 暗色模式支持 */
@media (prefers-color-scheme: dark) {
  .payment-method-dialog {
    --card-bg: #2d2d2d;
    --text-primary: #e6e6e6;
    --text-secondary: #999999;
    --bg-secondary: #3a3a3a;
    --border-color: #3d3d3d;
    --header-bg: #1e3a3a;
  }
  
  .selected-plan-info {
    background: var(--bg-secondary, #3a3a3a);
  }
  
  .payment-method-card {
    background: var(--card-bg, #2d2d2d);
  }
  
  .payment-method-card.selected {
    background: rgba(44, 122, 123, 0.15);
  }
}
</style>