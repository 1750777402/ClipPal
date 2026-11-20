import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  reactive,
  ref,
  type Ref,
  watch,
} from 'vue'

interface VirtualItem<T> {
  item: T
  index: number
  offsetY: number
  height: number
}

interface VirtualScrollOptions {
  estimatedItemHeight?: number
  overscan?: number
}

/** Variable-height virtual list. Only visible items plus an overscan window are mounted. */
export function useDynamicVirtualScroll<T extends { id: string }>(
  items: Ref<T[]>,
  containerRef: Ref<HTMLElement | null>,
  options: VirtualScrollOptions = {},
) {
  const estimatedItemHeight = options.estimatedItemHeight ?? 180
  const overscan = options.overscan ?? 600
  const scrollTop = ref(0)
  const viewportHeight = ref(600)
  const heightCache = reactive(new Map<string, number>())
  let resizeObserver: ResizeObserver | null = null
  let scrollFrame: number | null = null
  let attachedElement: HTMLElement | null = null

  const offsets = computed(() => {
    const values: number[] = [0]
    for (const item of items.value) {
      values.push(values[values.length - 1] + (heightCache.get(item.id) ?? estimatedItemHeight))
    }
    return values
  })

  const totalHeight = computed(() => offsets.value[offsets.value.length - 1] ?? 0)

  const findIndexAtOffset = (offset: number) => {
    const values = offsets.value
    let low = 0
    let high = Math.max(0, values.length - 1)

    while (low < high) {
      const middle = Math.floor((low + high) / 2)
      if (values[middle] <= offset) low = middle + 1
      else high = middle
    }

    return Math.max(0, Math.min(items.value.length, low - 1))
  }

  const visibleRange = computed(() => {
    if (items.value.length === 0) return { startIndex: 0, endIndex: 0 }

    const startIndex = Math.max(0, findIndexAtOffset(Math.max(0, scrollTop.value - overscan)))
    const endIndex = Math.min(
      items.value.length,
      findIndexAtOffset(scrollTop.value + viewportHeight.value + overscan) + 1,
    )

    return { startIndex, endIndex: Math.max(startIndex + 1, endIndex) }
  })

  const visibleItems = computed<VirtualItem<T>[]>(() => {
    const { startIndex, endIndex } = visibleRange.value
    return items.value.slice(startIndex, endIndex).map((item, relativeIndex) => {
      const index = startIndex + relativeIndex
      return {
        item,
        index,
        offsetY: offsets.value[index],
        height: heightCache.get(item.id) ?? estimatedItemHeight,
      }
    })
  })

  const updateViewport = () => {
    const element = containerRef.value
    if (element) viewportHeight.value = element.clientHeight || viewportHeight.value
  }

  const handleScroll = () => {
    if (scrollFrame !== null) return
    scrollFrame = requestAnimationFrame(() => {
      scrollFrame = null
      const element = containerRef.value
      if (element) scrollTop.value = element.scrollTop
    })
  }

  const observeVisibleItems = async () => {
    await nextTick()
    const element = containerRef.value
    if (!element) return

    resizeObserver?.disconnect()
    resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const card = entry.target as HTMLElement
        if (card === element) {
          updateViewport()
          continue
        }
        const id = card.dataset.itemId
        if (!id) continue

        const marginBottom = Number.parseFloat(getComputedStyle(card).marginBottom) || 0
        const nextHeight = Math.ceil(entry.contentRect.height + marginBottom)
        if (nextHeight > 0 && heightCache.get(id) !== nextHeight) {
          heightCache.set(id, nextHeight)
        }
      }
    })

    resizeObserver.observe(element)
    element.querySelectorAll<HTMLElement>('[data-item-id]').forEach((card) => {
      resizeObserver?.observe(card)
    })
  }

  const attachScrollListener = () => {
    const element = containerRef.value
    if (!element) return
    attachedElement = element
    updateViewport()
    element.addEventListener('scroll', handleScroll, { passive: true })
    observeVisibleItems()
  }

  const detachScrollListener = () => {
    attachedElement?.removeEventListener('scroll', handleScroll)
    attachedElement = null
    resizeObserver?.disconnect()
    resizeObserver = null
    if (scrollFrame !== null) {
      cancelAnimationFrame(scrollFrame)
      scrollFrame = null
    }
  }

  onMounted(attachScrollListener)
  onBeforeUnmount(detachScrollListener)

  watch(containerRef, () => {
    detachScrollListener()
    nextTick(attachScrollListener)
  })
  watch(visibleItems, observeVisibleItems, { flush: 'post' })

  const scrollToItem = (itemId: string, behavior: ScrollBehavior = 'smooth') => {
    const index = items.value.findIndex((item) => item.id === itemId)
    const element = containerRef.value
    if (index < 0 || !element) return
    element.scrollTo({ top: offsets.value[index], behavior })
  }

  return {
    visibleItems,
    visibleRange,
    totalHeight,
    scrollToItem,
    heightCache,
  }
}
