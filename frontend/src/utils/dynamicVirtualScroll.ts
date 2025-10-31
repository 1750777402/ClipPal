import { ref, computed, onMounted, onUnmounted, watch } from 'vue'

/**
 * 动态高度虚拟滚动 - 支持不同高度的卡片
 * 
 * 原理：
 * 1. 维护一个高度缓存 Map，记录每个 item 的实际高度
 * 2. 使用 ResizeObserver 监听卡片高度变化，自动更新缓存
 * 3. 根据缓存计算可见范围
 * 4. 只渲染可见范围内的卡片
 */
export function useDynamicVirtualScroll<T extends { id: string }>(
  items: T[],
  containerHeight: number = 600,
  overscan: number = 3
) {
  const scrollTop = ref(0)
  const containerRef = ref<HTMLElement>()
  
  // 高度缓存：记录每个 item 的实际高度
  const heightCache = new Map<string, number>()
  
  // 默认高度（用于未测量的卡片）
  const DEFAULT_ITEM_HEIGHT = 120
  
  // 获取 item 的高度（优先使用缓存，否则使用默认值）
  const getItemHeight = (item: T): number => {
    return heightCache.get(item.id) || DEFAULT_ITEM_HEIGHT
  }
  
  // 计算所有 item 的累积高度
  const getAccumulativeHeight = (index: number): number => {
    let height = 0
    for (let i = 0; i < index; i++) {
      height += getItemHeight(items[i])
    }
    return height
  }
  
  // 计算可见范围
  const visibleRange = computed(() => {
    let startIndex = 0
    let accumulativeHeight = 0
    
    // 二分查找起始索引
    for (let i = 0; i < items.length; i++) {
      const itemHeight = getItemHeight(items[i])
      if (accumulativeHeight + itemHeight > scrollTop.value) {
        startIndex = Math.max(0, i - overscan)
        break
      }
      accumulativeHeight += itemHeight
    }
    
    // 计算结束索引
    let endIndex = startIndex
    let visibleHeight = 0
    for (let i = startIndex; i < items.length; i++) {
      const itemHeight = getItemHeight(items[i])
      if (visibleHeight > scrollTop.value + containerHeight + (overscan * DEFAULT_ITEM_HEIGHT)) {
        break
      }
      visibleHeight += itemHeight
      endIndex = i + 1
    }
    
    return {
      startIndex,
      endIndex,
      total: items.length
    }
  })
  
  // 可见的 items 及其位置信息
  const visibleItems = computed(() => {
    const { startIndex, endIndex } = visibleRange.value
    const result = []
    
    for (let i = startIndex; i < endIndex; i++) {
      const item = items[i]
      const offsetY = getAccumulativeHeight(i)
      
      result.push({
        item,
        index: i,
        offsetY,
        height: getItemHeight(item)
      })
    }
    
    return result
  })
  
  // 容器总高度
  const totalHeight = computed(() => {
    return items.reduce((sum, item) => sum + getItemHeight(item), 0)
  })
  
  // 滚动处理
  const handleScroll = (event: Event) => {
    const target = event.target as HTMLElement
    scrollTop.value = target.scrollTop
  }
  
  // 使用 ResizeObserver 监听卡片高度变化
  let resizeObserver: ResizeObserver | null = null
  
  const setupResizeObserver = () => {
    if (!containerRef.value) return
    
    resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const element = entry.target as HTMLElement
        const itemId = element.getAttribute('data-item-id')
        
        if (itemId) {
          const newHeight = element.offsetHeight
          const oldHeight = heightCache.get(itemId)
          
          // 只在高度变化时更新缓存
          if (oldHeight !== newHeight) {
            heightCache.set(itemId, newHeight)
          }
        }
      }
    })
    
    // 监听所有卡片元素
    const cardElements = containerRef.value.querySelectorAll('[data-item-id]')
    cardElements.forEach(element => {
      resizeObserver?.observe(element)
    })
  }
  
  // 清理函数
  let scrollCleanup: (() => void) | null = null
  
  onMounted(() => {
    if (containerRef.value) {
      containerRef.value.addEventListener('scroll', handleScroll, { passive: true })
      scrollCleanup = () => {
        containerRef.value?.removeEventListener('scroll', handleScroll)
      }
      
      // 延迟设置 ResizeObserver，确保 DOM 已挂载
      setTimeout(setupResizeObserver, 100)
    }
  })
  
  onUnmounted(() => {
    scrollCleanup?.()
    resizeObserver?.disconnect()
  })
  
  // 监听 items 变化，重新设置 ResizeObserver
  watch(
    () => items.length,
    () => {
      setTimeout(setupResizeObserver, 100)
    }
  )
  
  // 滚动到指定 item
  const scrollToItem = (itemId: string, behavior: ScrollBehavior = 'smooth') => {
    if (!containerRef.value) return
    
    const index = items.findIndex(item => item.id === itemId)
    if (index === -1) return
    
    const offsetY = getAccumulativeHeight(index)
    containerRef.value.scrollTo({
      top: offsetY,
      behavior
    })
  }
  
  return {
    containerRef,
    visibleItems,
    totalHeight,
    visibleRange,
    scrollToItem,
    heightCache
  }
}

