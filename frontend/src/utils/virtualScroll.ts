import { ref, computed, onMounted, onUnmounted } from 'vue'

/**
 * 虚拟滚动Hook - 只渲染可见的DOM元素，大幅减少内存占用
 */
export function useVirtualScroll<T>(
  items: T[],
  itemHeight: number = 120, // 每个item的大概高度
  containerHeight: number = 600, // 容器高度
  overscan: number = 5 // 额外渲染的item数量
) {
  const scrollTop = ref(0)
  const containerRef = ref<HTMLElement>()
  
  // 计算可见区域
  const visibleRange = computed(() => {
    const start = Math.floor(scrollTop.value / itemHeight)
    const visibleCount = Math.ceil(containerHeight / itemHeight)
    const end = Math.min(start + visibleCount + overscan, items.length)
    
    return {
      start: Math.max(0, start - overscan),
      end,
      total: items.length
    }
  })
  
  // 可见的items
  const visibleItems = computed(() => {
    const { start, end } = visibleRange.value
    return items.slice(start, end).map((item, index) => ({
      item,
      index: start + index,
      offsetY: (start + index) * itemHeight
    }))
  })
  
  // 容器总高度
  const totalHeight = computed(() => items.length * itemHeight)
  
  // 滚动处理
  const handleScroll = (event: Event) => {
    const target = event.target as HTMLElement
    scrollTop.value = target.scrollTop
  }
  
  // 清理函数
  let cleanup: (() => void) | null = null
  
  onMounted(() => {
    if (containerRef.value) {
      containerRef.value.addEventListener('scroll', handleScroll, { passive: true })
      cleanup = () => {
        containerRef.value?.removeEventListener('scroll', handleScroll)
      }
    }
  })
  
  onUnmounted(() => {
    cleanup?.()
  })
  
  return {
    containerRef,
    visibleItems,
    totalHeight,
    visibleRange
  }
}

/**
 * 轻量级虚拟滚动 - 仅处理大量数据时的性能问题
 * 当items数量 < 50时不启用虚拟滚动，避免不必要的复杂性
 */
export function useSmartVirtualScroll<T>(
  items: T[],
  itemHeight: number = 120,
  containerHeight: number = 600
) {
  const shouldUseVirtual = computed(() => items.length > 50)
  
  const virtualScroll = useVirtualScroll(items, itemHeight, containerHeight)
  
  // 当不需要虚拟滚动时，返回所有items
  const displayItems = computed(() => {
    if (!shouldUseVirtual.value) {
      return items.map((item, index) => ({
        item,
        index,
        offsetY: 0
      }))
    }
    return virtualScroll.visibleItems.value
  })
  
  return {
    ...virtualScroll,
    displayItems,
    shouldUseVirtual
  }
}