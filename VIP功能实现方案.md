# Clip-Pal VIP功能实现方案

**文档版本**: v1.0  
**创建时间**: 2025-01-28  
**项目版本**: v1.0.5

## 📋 需求概述

### 业务需求
- **目标用户**: 个人用户、企业用户、程序员等常用复制粘贴的用户
- **付费模式**: 月费¥6、季费¥15、年费¥50
- **免费限制**: 完全禁用云同步或仅支持10条文本内容云同步体验
- **VIP权益**: 开启VIP后，本地记录条数与云同步条数联动

### 功能权限对比

| 功能特性 | 免费用户 | VIP用户 |
|---------|----------|---------|
| 本地记录存储 | 最多500条 | 最多1000条 |
| 云同步功能 | 仅10条体验 | 完整1000条 |
| 文件同步 | 不支持 | 支持5MB以下 |
| 多设备同步 | 不支持 | 完整支持 |
| 数据备份 | 仅本地 | 云端+本地 |

## 🏗️ 技术架构

### 加密存储架构
基于现有的`SecureStore`机制，VIP信息使用AES-256-GCM加密存储，确保数据安全性。

### 权限检查流程
```
用户操作 → 登录状态检查 → VIP权限验证 → 功能限制检查 → 执行/拒绝操作
```

### 核心模块设计
- **VipChecker**: 权限检查器，处理所有VIP相关验证
- **SecureStore**: 扩展现有加密存储，增加VIP信息字段
- **VipManagement**: Tauri命令层，提供前端API接口
- **VipStore**: 前端状态管理，响应式VIP状态控制

## 📦 后端实现(Rust)

### 1. 数据结构定义

```rust
// src-tauri/src/utils/secure_store.rs

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct SecureData {
    // 现有字段
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub user_info: Option<String>,
    pub token_expires: Option<i32>,
    
    // 新增VIP相关字段
    pub vip_info: Option<String>,           // JSON序列化的VIP信息
    pub vip_last_check: Option<u64>,        // 上次检查VIP状态的时间戳
    pub server_config: Option<String>,      // 服务器配置信息
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VipInfo {
    pub is_vip: bool,
    pub vip_type: VipType,
    pub expire_time: Option<u64>,          // 到期时间戳
    pub max_records: u32,                  // 最大记录数限制
    pub max_sync_records: u32,             // 可云同步的最大记录数  
    pub features: Vec<String>,             // VIP功能列表
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VipType {
    Free,     // 免费用户
    Monthly,  // 月付费
    Quarterly,// 季度付费  
    Yearly,   // 年付费
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub max_file_size: u64,               // 服务器控制的文件大小限制
    pub free_sync_limit: u32,             // 免费用户云同步限制
    pub vip_sync_limit: u32,              // VIP用户云同步限制
}
```

### 2. SecureStore扩展方法

```rust
impl SecureStore {
    /// 获取VIP信息
    pub fn get_vip_info(&mut self) -> AppResult<Option<VipInfo>> {
        if !self.loaded {
            self.load()?;
        }
        
        if let Some(vip_str) = &self.data.vip_info {
            let vip_info: VipInfo = serde_json::from_str(vip_str)
                .map_err(|e| AppError::Serde(format!("VIP信息反序列化失败: {}", e)))?;
            Ok(Some(vip_info))
        } else {
            Ok(None)
        }
    }

    /// 设置VIP信息并自动保存
    pub fn set_vip_info(&mut self, vip_info: VipInfo) -> AppResult<()> {
        if !self.loaded {
            self.load()?;
        }
        
        let vip_str = serde_json::to_string(&vip_info)
            .map_err(|e| AppError::Serde(format!("VIP信息序列化失败: {}", e)))?;
            
        self.data.vip_info = Some(vip_str);
        self.save()
    }

    /// 清除VIP信息
    pub fn clear_vip_info(&mut self) -> AppResult<()> {
        if !self.loaded {
            self.load()?;
        }
        self.data.vip_info = None;
        self.data.vip_last_check = None;
        self.save()
    }

    /// 检查是否需要更新VIP状态(超过1小时)
    pub fn should_check_vip_status(&mut self) -> AppResult<bool> {
        if !self.loaded {
            self.load()?;
        }
        
        if let Some(last_check) = self.data.vip_last_check {
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            Ok(current_time - last_check > 3600) // 1小时
        } else {
            Ok(true) // 从未检查过
        }
    }
}
```

### 3. VIP权限检查器

```rust
// src-tauri/src/utils/vip_checker.rs

pub struct VipChecker;

impl VipChecker {
    /// 检查用户是否为VIP
    pub fn is_vip_user() -> AppResult<bool> {
        if let Some(vip_info) = Self::get_local_vip_info()? {
            if vip_info.is_vip {
                // 检查是否过期
                if let Some(expire_time) = vip_info.expire_time {
                    let current_time = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    return Ok(current_time < expire_time);
                }
                return Ok(true); // 永久VIP
            }
        }
        Ok(false)
    }

    /// 检查云同步权限
    pub fn check_cloud_sync_permission() -> AppResult<(bool, String)> {
        // 首先检查是否登录
        let mut store = SECURE_STORE.write()
            .map_err(|_| AppError::Config("获取存储锁失败".to_string()))?;
        
        if store.get_jwt_token()?.is_none() {
            return Ok((false, "需要登录后才能使用云同步功能".to_string()));
        }

        // 检查VIP状态
        if Self::is_vip_user()? {
            return Ok((true, "VIP用户，享受完整云同步功能".to_string()));
        }

        // 免费用户检查10条限制
        let current_sync_count = Self::get_current_sync_count()?;
        if current_sync_count < 10 {
            Ok((true, format!("免费体验，已使用 {}/10 条云同步", current_sync_count)))
        } else {
            Ok((false, "免费用户云同步额度已用完，请升级VIP".to_string()))
        }
    }

    /// 获取最大记录数限制
    pub fn get_max_records_limit() -> AppResult<u32> {
        if Self::is_vip_user()? {
            Ok(1000)
        } else {
            Ok(500)
        }
    }

    /// 验证设置的记录条数是否合法
    pub fn validate_max_records(max_records: u32) -> AppResult<()> {
        let limit = Self::get_max_records_limit()?;
        
        if max_records < 50 || max_records > limit {
            return Err(AppError::Config(
                format!("最大记录条数必须在50-{}之间", limit)
            ));
        }
        
        Ok(())
    }

    /// 重置为免费用户状态
    pub fn reset_to_free_user() -> AppResult<()> {
        log::info!("重置用户状态为免费用户");
        
        // 清除VIP信息
        let mut store = SECURE_STORE.write()
            .map_err(|_| AppError::Config("获取存储锁失败".to_string()))?;
        store.clear_vip_info()?;
        drop(store);

        // 更新系统设置
        let mut settings = load_settings().await?;
        settings.cloud_sync = 0; // 关闭云同步
        
        // 如果当前记录数超过免费限制，调整为500
        if settings.max_records > 500 {
            settings.max_records = 500;
        }
        
        save_settings(settings).await?;
        
        Ok(())
    }
}
```

### 4. Tauri命令接口

```rust
// src-tauri/src/biz/vip_management.rs

#[tauri::command]
pub async fn get_vip_status() -> AppResult<Option<VipInfo>> {
    VipChecker::get_local_vip_info()
}

#[tauri::command]
pub async fn check_vip_permission() -> AppResult<(bool, String)> {
    VipChecker::check_cloud_sync_permission()
}

#[tauri::command]
pub async fn get_vip_limits() -> AppResult<serde_json::Value> {
    let max_records = VipChecker::get_max_records_limit()?;
    let max_file_size = VipChecker::get_max_file_size()?;
    let is_vip = VipChecker::is_vip_user()?;
    
    Ok(serde_json::json!({
        "isVip": is_vip,
        "maxRecords": max_records,
        "maxFileSize": max_file_size,
        "canCloudSync": VipChecker::check_cloud_sync_permission()?.0
    }))
}

#[tauri::command]
pub async fn open_vip_purchase_page(app_handle: AppHandle) -> AppResult<()> {
    let url = "https://jingchuanyuexiang.com";
    
    // 使用系统命令打开浏览器
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| AppError::Config(format!("打开浏览器失败: {}", e)))?;
    }
    
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("cmd")
            .args(["/c", "start", url])
            .spawn()
            .map_err(|e| AppError::Config(format!("打开浏览器失败: {}", e)))?;
    }
    
    Ok(())
}

#[tauri::command]
pub async fn refresh_vip_status(app_handle: AppHandle) -> AppResult<bool> {
    // TODO: 调用服务端API检查VIP状态
    log::info!("刷新VIP状态 - 待实现服务端接口");
    Ok(false)
}

// 模拟VIP状态更新(用于测试)
#[tauri::command]
pub async fn simulate_vip_upgrade(
    app_handle: AppHandle,
    vip_type: VipType,
    days: u32
) -> AppResult<()> {
    let expire_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() + (days as u64 * 24 * 3600);
    
    let vip_info = VipInfo {
        is_vip: true,
        vip_type: vip_type.clone(),
        expire_time: Some(expire_time),
        max_records: 1000,
        max_sync_records: 1000,
        features: vec!["云同步".to_string(), "大文件上传".to_string()],
    };
    
    let mut store = SECURE_STORE.write()
        .map_err(|_| AppError::Config("获取存储锁失败".to_string()))?;
    store.set_vip_info(vip_info.clone())?;
    store.update_vip_check_time()?;
    drop(store);
    
    // 发送状态变更事件
    let payload = VipStatusChangedPayload {
        is_vip: true,
        vip_type: Some(vip_type),
        expire_time: Some(expire_time),
        max_records: 1000,
    };
    
    app_handle.emit("vip-status-changed", payload)
        .map_err(|e| AppError::Config(format!("发送事件失败: {}", e)))?;
    
    Ok(())
}
```

### 5. 系统设置验证更新

```rust
// src-tauri/src/biz/system_setting.rs

use crate::utils::vip_checker::VipChecker;

// 更新验证设置的有效性函数
fn validate_settings(settings: &Settings) -> AppResult<()> {
    // 使用VIP检查器验证记录条数
    VipChecker::validate_max_records(settings.max_records)?;
    
    // 其他验证逻辑保持不变...
    Ok(())
}
```

## 🎨 前端实现(Vue.js + TypeScript)

### 1. VIP状态管理

```typescript
// frontend/src/utils/vipStore.ts

export interface VipInfo {
  isVip: boolean;
  vipType: 'Free' | 'Monthly' | 'Quarterly' | 'Yearly';
  expireTime?: number;
  maxRecords: number;
  maxSyncRecords: number;
  features: string[];
}

export interface VipLimits {
  isVip: boolean;
  maxRecords: number;
  maxFileSize: number;
  canCloudSync: boolean;
}

const vipState = reactive({
  vipInfo: null as VipInfo | null,
  limits: null as VipLimits | null,
  loading: false,
});

export const vipStore = {
  // 状态
  get vipInfo() { return vipState.vipInfo; },
  get limits() { return vipState.limits; },
  get loading() { return vipState.loading; },

  // 计算属性
  isVip: computed(() => vipState.vipInfo?.isVip ?? false),
  canCloudSync: computed(() => vipState.limits?.canCloudSync ?? false),
  maxRecordsLimit: computed(() => vipState.limits?.maxRecords ?? 500),

  // VIP类型显示名称
  vipTypeDisplay: computed(() => {
    switch (vipState.vipInfo?.vipType) {
      case 'Monthly': return '月度会员';
      case 'Quarterly': return '季度会员';
      case 'Yearly': return '年度会员';
      default: return '免费用户';
    }
  }),

  // 初始化VIP状态
  async init() {
    vipState.loading = true;
    try {
      await Promise.all([
        this.loadVipStatus(),
        this.loadVipLimits()
      ]);
      
      // 监听VIP状态变更事件
      await listen('vip-status-changed', (event: any) => {
        console.log('VIP状态已变更:', event.payload);
        this.loadVipStatus();
        this.loadVipLimits();
      });
      
    } catch (error) {
      console.error('初始化VIP状态失败:', error);
    } finally {
      vipState.loading = false;
    }
  },

  // 检查云同步权限
  async checkCloudSyncPermission() {
    try {
      const response = await apiInvoke<[boolean, string]>('check_vip_permission');
      if (isSuccess(response)) {
        return {
          allowed: response.data[0],
          message: response.data[1]
        };
      }
    } catch (error) {
      console.error('检查云同步权限失败:', error);
    }
    return { allowed: false, message: '权限检查失败' };
  },

  // 打开VIP购买页面
  async openPurchasePage() {
    try {
      await apiInvoke('open_vip_purchase_page');
    } catch (error) {
      console.error('打开购买页面失败:', error);
      throw error;
    }
  },

  // 刷新VIP状态
  async refreshStatus() {
    vipState.loading = true;
    try {
      const response = await apiInvoke<boolean>('refresh_vip_status');
      if (isSuccess(response) && response.data) {
        await this.loadVipStatus();
        await this.loadVipLimits();
        return true;
      }
    } catch (error) {
      console.error('刷新VIP状态失败:', error);
    } finally {
      vipState.loading = false;
    }
    return false;
  }
};
```

### 2. VIP购买对话框组件

```vue
<!-- frontend/src/components/VipUpgradeDialog.vue -->
<template>
  <div class="vip-upgrade-modal" v-if="visible" @click.self="$emit('close')">
    <div class="vip-dialog">
      <div class="dialog-header">
        <h2>升级VIP会员</h2>
        <button class="close-btn" @click="$emit('close')">×</button>
      </div>
      
      <!-- 当前状态显示 -->
      <div class="current-status" v-if="vipStore.vipInfo">
        <div class="status-card" :class="{ 'vip-active': vipStore.isVip }">
          <div class="status-icon">{{ vipStore.isVip ? '👑' : '🆓' }}</div>
          <div class="status-info">
            <div class="status-title">{{ vipStore.vipTypeDisplay }}</div>
            <div class="status-detail" v-if="vipStore.isVip && vipStore.expireTimeDisplay">
              到期时间: {{ vipStore.expireTimeDisplay }}
            </div>
          </div>
        </div>
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
            
            <button class="plan-button" @click="handlePurchase(plan.type)">
              {{ plan.buttonText }}
            </button>
          </div>
        </div>
      </div>

      <!-- 功能对比表 -->
      <div class="comparison-section">
        <h3>功能对比</h3>
        <div class="comparison-table">
          <div class="comparison-header">
            <div class="feature-col">功能</div>
            <div class="free-col">免费版</div>
            <div class="vip-col">VIP版</div>
          </div>
          <div class="comparison-row" v-for="comparison in featureComparisons" :key="comparison.feature">
            <div class="feature-col">{{ comparison.feature }}</div>
            <div class="free-col">{{ comparison.free }}</div>
            <div class="vip-col">{{ comparison.vip }}</div>
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
          <button class="refresh-btn" @click="handleRefreshStatus">
            {{ vipStore.loading ? '检查中...' : '刷新VIP状态' }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { vipStore } from '../utils/vipStore';

const showPurchaseGuide = ref(false);

const vipPlans = [
  {
    type: 'Monthly',
    title: '月度会员',
    price: 6,
    period: '月',
    features: ['1000条记录存储', '1000条云同步', '5MB文件上传', '多设备同步'],
    buttonText: '开通月度会员',
    recommended: false
  },
  {
    type: 'Quarterly', 
    title: '季度会员',
    price: 15,
    period: '3个月',
    features: ['1000条记录存储', '1000条云同步', '5MB文件上传', '多设备同步', '季度优惠价'],
    buttonText: '开通季度会员',
    recommended: true
  },
  {
    type: 'Yearly',
    title: '年度会员',
    price: 50,
    period: '12个月',
    features: ['1000条记录存储', '1000条云同步', '5MB文件上传', '多设备同步', '年度超值价'],
    buttonText: '开通年度会员',
    recommended: false
  }
];

const featureComparisons = [
  { feature: '本地记录存储', free: '最多500条', vip: '最多1000条' },
  { feature: '云同步功能', free: '仅10条体验', vip: '完整1000条' },
  { feature: '文件同步', free: '不支持', vip: '支持5MB以下' },
  { feature: '多设备同步', free: '不支持', vip: '完整支持' }
];

const handlePurchase = async (planType: string) => {
  try {
    await vipStore.openPurchasePage();
    showPurchaseGuide.value = true;
  } catch (error) {
    console.error('打开购买页面失败:', error);
  }
};

const handleRefreshStatus = async () => {
  try {
    const updated = await vipStore.refreshStatus();
    if (updated) {
      showPurchaseGuide.value = false;
    }
  } catch (error) {
    console.error('刷新状态失败:', error);
  }
};
</script>
```

## 🔄 集成步骤

### 第一阶段：基础架构搭建

1. **创建新文件**：
   - `src-tauri/src/utils/vip_checker.rs`
   - `src-tauri/src/biz/vip_management.rs`

2. **修改现有文件**：
   - 扩展 `src-tauri/src/utils/secure_store.rs`
   - 更新 `src-tauri/src/biz/system_setting.rs`
   - 注册命令到 `src-tauri/src/lib.rs`

3. **前端文件**：
   - `frontend/src/utils/vipStore.ts`
   - `frontend/src/components/VipUpgradeDialog.vue`

### 第二阶段：权限控制集成

1. **云同步权限检查**：
   - 在 `cloud_sync_timer.rs` 中集成VIP检查
   - 添加权限验证到所有云同步相关API

2. **设置页面集成**：
   - 显示VIP状态和限制信息
   - 集成VIP购买入口

3. **用户界面优化**：
   - 添加VIP状态指示器
   - 非VIP用户的功能引导提示

### 第三阶段：测试与优化

1. **功能测试**：
   - VIP权限检查逻辑测试
   - 加密存储读写测试
   - 前端状态管理测试

2. **边界情况处理**：
   - 网络异常时的本地缓存策略
   - VIP过期时的优雅降级
   - 权限检查失败的用户提示

## 📝 开发注意事项

### 安全考虑
- VIP信息使用与用户认证信息相同的AES-256-GCM加密
- 所有权限检查都有服务端二次验证
- 敏感操作需要实时权限校验

### 性能优化
- VIP状态本地缓存，减少服务端请求
- 权限检查结果缓存，避免重复计算
- 前端响应式状态管理，减少不必要的重渲染

### 用户体验
- 渐进式功能引导，避免突然的功能限制
- 清晰的VIP价值展示，提高转化率
- 优雅的错误处理和用户反馈

### 扩展性设计
- 支持多种VIP等级扩展
- 服务端配置控制，无需客户端更新
- 模块化设计，便于功能增减

## 🚀 后续服务端对接

### API接口规范
```rust
// 待实现的服务端API
GET /api/vip/status          // 获取用户VIP状态
POST /api/vip/check          // 实时权限验证
GET /api/vip/config          // 获取服务端配置
POST /api/payment/create     // 创建支付订单
GET /api/payment/status/:id  // 查询支付状态
```

### 事件通知机制
- VIP状态变更时发送前端事件
- 权限验证失败时的状态重置
- 支付完成后的状态更新通知

这个方案为你的Clip-Pal项目提供了完整的VIP功能基础架构，可以根据实际需要进行调整和扩展。