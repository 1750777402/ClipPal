import { ref, onMounted, onBeforeUnmount } from 'vue'

// 响应式断点定义
export const BREAKPOINTS = {
    xs: 360,      // 极小窗口
    sm: 480,      // 小窗口
    md: 600,      // 中等窗口
    lg: 768,      // 大窗口
    xl: 1024,     // 超大窗口
    // Tauri特殊尺寸
    tauriNarrow: 500,  // Tauri窄窗口模式
    tauriWide: 800     // Tauri宽窗口模式
} as const

// 设备类型
export type DeviceType = 'mobile' | 'tablet' | 'desktop' | 'tauri-narrow' | 'tauri-wide'

// 平台类型
export type PlatformType = 'windows' | 'macos' | 'linux' | 'unknown'

// 检测平台
export function detectPlatform(): PlatformType {
    const platform = navigator.platform.toLowerCase()
    const userAgent = navigator.userAgent.toLowerCase()

    if (platform.includes('mac') || userAgent.includes('mac')) {
        return 'macos'
    } else if (platform.includes('win') || userAgent.includes('win')) {
        return 'windows'
    } else if (platform.includes('linux') || userAgent.includes('linux')) {
        return 'linux'
    }

    return 'unknown'
}

// 检测设备类型
export function detectDeviceType(width: number, height: number): DeviceType {
    // Tauri窗口特殊判断
    if (width >= 400 && width <= BREAKPOINTS.tauriNarrow && height >= 600) {
        return 'tauri-narrow'
    }

    if (width > BREAKPOINTS.tauriNarrow && width <= BREAKPOINTS.tauriWide) {
        return 'tauri-wide'
    }

    // 常规设备判断
    if (width <= BREAKPOINTS.sm) {
        return 'mobile'
    } else if (width <= BREAKPOINTS.lg) {
        return 'tablet'
    } else {
        return 'desktop'
    }
}

// 检测是否支持backdrop-filter (macOS Safari特性)
export function supportsBackdropFilter(): boolean {
    return CSS.supports('-webkit-backdrop-filter', 'blur(1px)') ||
        CSS.supports('backdrop-filter', 'blur(1px)')
}

// 检测是否为高DPI屏幕
export function isHighDPI(): boolean {
    return window.devicePixelRatio > 1.5
}

// 检测是否为触摸设备
export function isTouchDevice(): boolean {
    return 'ontouchstart' in window || navigator.maxTouchPoints > 0
}

// 响应式断点Hook
export function useBreakpoint() {
    const width = ref(window.innerWidth)
    const height = ref(window.innerHeight)
    const deviceType = ref<DeviceType>(detectDeviceType(width.value, height.value))
    const platform = ref<PlatformType>(detectPlatform())

    // 计算属性：各种断点判断
    const isXs = ref(width.value <= BREAKPOINTS.xs)
    const isSm = ref(width.value <= BREAKPOINTS.sm)
    const isMd = ref(width.value <= BREAKPOINTS.md)
    const isLg = ref(width.value <= BREAKPOINTS.lg)
    const isXl = ref(width.value <= BREAKPOINTS.xl)

    const isMobile = ref(deviceType.value === 'mobile')
    const isTablet = ref(deviceType.value === 'tablet')
    const isDesktop = ref(deviceType.value === 'desktop')
    const isTauriNarrow = ref(deviceType.value === 'tauri-narrow')
    const isTauriWide = ref(deviceType.value === 'tauri-wide')

    const isMac = ref(platform.value === 'macos')
    const isWindows = ref(platform.value === 'windows')
    const isLinux = ref(platform.value === 'linux')

    const supportsBlur = ref(supportsBackdropFilter())
    const isRetina = ref(isHighDPI())
    const hasTouch = ref(isTouchDevice())

    // 防抖更新函数
    let resizeTimer: ReturnType<typeof setTimeout> | null = null

    const updateBreakpoints = () => {
        width.value = window.innerWidth
        height.value = window.innerHeight

        const newDeviceType = detectDeviceType(width.value, height.value)
        deviceType.value = newDeviceType

        // 更新断点
        isXs.value = width.value <= BREAKPOINTS.xs
        isSm.value = width.value <= BREAKPOINTS.sm
        isMd.value = width.value <= BREAKPOINTS.md
        isLg.value = width.value <= BREAKPOINTS.lg
        isXl.value = width.value <= BREAKPOINTS.xl

        // 更新设备类型
        isMobile.value = newDeviceType === 'mobile'
        isTablet.value = newDeviceType === 'tablet'
        isDesktop.value = newDeviceType === 'desktop'
        isTauriNarrow.value = newDeviceType === 'tauri-narrow'
        isTauriWide.value = newDeviceType === 'tauri-wide'
    }

    const handleResize = () => {
        if (resizeTimer) clearTimeout(resizeTimer)
        resizeTimer = setTimeout(updateBreakpoints, 150)
    }

    onMounted(() => {
        window.addEventListener('resize', handleResize)
        updateBreakpoints() // 初始化
    })

    onBeforeUnmount(() => {
        window.removeEventListener('resize', handleResize)
        if (resizeTimer) clearTimeout(resizeTimer)
    })

    return {
        // 尺寸
        width: width.value,
        height: height.value,

        // 断点
        isXs,
        isSm,
        isMd,
        isLg,
        isXl,

        // 设备类型
        deviceType,
        isMobile,
        isTablet,
        isDesktop,
        isTauriNarrow,
        isTauriWide,

        // 平台
        platform,
        isMac,
        isWindows,
        isLinux,

        // 特性检测
        supportsBlur,
        isRetina,
        hasTouch,

        // 工具函数
        updateBreakpoints
    }
}

// 窗口大小适应Hook - 针对Tauri应用优化
export function useWindowAdaptive() {
    const breakpoint = useBreakpoint()

    // 根据窗口大小调整字体
    const getFontScale = (): number => {
        if (breakpoint.isXs.value) return 0.85
        if (breakpoint.isSm.value) return 0.9
        if (breakpoint.isTauriNarrow.value) return 0.95
        return 1
    }

    // 根据窗口大小调整间距
    const getSpaceScale = (): number => {
        if (breakpoint.isXs.value) return 0.75
        if (breakpoint.isSm.value) return 0.85
        if (breakpoint.isTauriNarrow.value) return 0.9
        return 1
    }

    // 获取合适的卡片边距
    const getCardMargin = (): string => {
        if (breakpoint.isXs.value) return '0 6px 8px 6px'
        if (breakpoint.isSm.value) return '0 8px 10px 8px'
        if (breakpoint.isMd.value) return '0 12px 12px 12px'
        if (breakpoint.isTauriNarrow.value) return '0 16px 14px 16px'
        return '0 20px 16px 20px'
    }

    // 获取合适的弹窗尺寸
    const getDialogSize = (): { width: string; maxWidth: string; minWidth: string } => {
        if (breakpoint.isXs.value) {
            return { width: '98%', maxWidth: '320px', minWidth: '280px' }
        } else if (breakpoint.isSm.value) {
            return { width: '92%', maxWidth: '360px', minWidth: '300px' }
        } else if (breakpoint.isMd.value) {
            return { width: '90%', maxWidth: '400px', minWidth: '320px' }
        } else if (breakpoint.isTauriNarrow.value) {
            return { width: '88%', maxWidth: '450px', minWidth: '380px' }
        }
        return { width: '85%', maxWidth: '480px', minWidth: '360px' }
    }

    return {
        ...breakpoint,
        getFontScale,
        getSpaceScale,
        getCardMargin,
        getDialogSize
    }
}

// CSS类生成器
export function generateResponsiveClasses(breakpoint: ReturnType<typeof useBreakpoint>): string[] {
    const classes: string[] = []

    // 设备类型类
    classes.push(`device-${breakpoint.deviceType.value}`)

    // 平台类
    classes.push(`platform-${breakpoint.platform.value}`)

    // 断点类
    if (breakpoint.isXs.value) classes.push('bp-xs')
    if (breakpoint.isSm.value) classes.push('bp-sm')
    if (breakpoint.isMd.value) classes.push('bp-md')
    if (breakpoint.isLg.value) classes.push('bp-lg')
    if (breakpoint.isXl.value) classes.push('bp-xl')

    // 特性类
    if (breakpoint.supportsBlur.value) classes.push('supports-blur')
    if (breakpoint.isRetina.value) classes.push('retina')
    if (breakpoint.hasTouch.value) classes.push('touch')

    return classes
} 