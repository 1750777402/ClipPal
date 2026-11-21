<template>
  <button class="icon-button iconfont icon-user" :class="{ 'logged-in': userStore.isLoggedIn() }"
    :title="userStore.isLoggedIn() ? '用户信息' : '点击登录'" type="button" @click="openAccount"></button>
  <LoginDialog v-if="showLogin" v-model:visible="showLogin" @login-success="loginSuccess" />
  <UserMenu v-if="showMenu" v-model:visible="showMenu" :user-info="userStore.getUserInfo()"
    @logout="logout" @user-info="showInfo = true" @vip-account="showVip = true" />
  <UserInfoDialog v-if="showInfo" v-model:visible="showInfo" :user-info="userStore.getUserInfo()" />
  <VipAccountDialog v-if="showVip" v-model:visible="showVip" @login="showLogin = true" />
</template>

<script setup lang="ts">
// 商业界面只由 commercial 前端入口动态加载，单机/自建构建不包含此模块。
import { ref, onMounted, onBeforeUnmount } from 'vue';
import { listen } from '@tauri-apps/api/event';
import LoginDialog from '../components/LoginDialog.vue';
import UserMenu from '../components/UserMenu.vue';
import UserInfoDialog from '../components/UserInfoDialog.vue';
import VipAccountDialog from '../components/VipAccountDialog.vue';
import { useUserStore } from '../utils/userStore';
import { useVipStore } from '../utils/vipStore';
// 账号和权益由专门的状态仓库维护，此组件只控制按钮与弹窗。
const userStore=useUserStore();const vipStore=useVipStore();
// 每个弹窗独立开关，不把商业状态混入本地记录列表。
const showLogin=ref(false);const showMenu=ref(false);const showInfo=ref(false);const showVip=ref(false);
const emit=defineEmits<{change:[]}>();
// 异步注册完成时组件可能已卸载；此标记防止遗留后台事件监听。
let disposed=false;const unlisteners:Array<()=>void>=[];
/** 已登录时打开用户菜单，否则打开登录弹窗。 */
function openAccount(){if(userStore.isLoggedIn())showMenu.value=true;else showLogin.value=true;}
/** 后端已保存登录凭据，这里只更新展示状态并通知列表刷新。 */
function loginSuccess(data:any){userStore.setLoginState(data.user_info);showLogin.value=false;emit('change');}
/** 通过状态仓库完成退出，再通知父组件更新页面。 */
async function logout(){await userStore.logout();emit('change');}
// 挂载后监听账号失效，再加载已有会话；不由该组件启动同步任务。
onMounted(async()=>{
  for(const event of ['auth-expired','auth-cleared']){
    const unlisten=await listen(event,()=>{userStore.clearLoginState();vipStore.clearVipState();});
    // 监听注册是异步操作；若期间卸载，拿到注销函数后立即释放。
    if(disposed)unlisten();else unlisteners.push(unlisten);
  }
  if(!disposed){await userStore.initialize();await vipStore.initialize();}
});
// 离开页面时清理全部监听，防止重复挂载后多次响应同一事件。
onBeforeUnmount(()=>{disposed=true;unlisteners.forEach(unlisten=>unlisten());});
</script>

<style scoped>
.icon-button { background: none; border: none; cursor: pointer; font-size: 20px; padding: 8px; color: var(--text-secondary); }
.icon-button:hover { color: var(--primary-color); }
.logged-in { color: var(--primary-color); }
</style>
