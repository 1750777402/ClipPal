import { onUnmounted } from 'vue'

/**
 * 内存管理器 - 防止内存泄漏的工具类
 * 自动跟踪和清理定时器、事件监听器、观察者等资源
 */
export class MemoryManager {
  private timers = new Set<ReturnType<typeof setTimeout>>()
  private intervals = new Set<ReturnType<typeof setInterval>>()
  private observers = new Set<IntersectionObserver | MutationObserver | ResizeObserver>()
  private eventCleanups = new Set<() => void>()
  private imageInstances = new Set<HTMLImageElement>()

  /**
   * 创建受管理的setTimeout
   */
  setTimeout(callback: () => void, delay: number): ReturnType<typeof setTimeout> {
    const timer = setTimeout(() => {
      this.timers.delete(timer)
      callback()
    }, delay)
    this.timers.add(timer)
    return timer
  }

  /**
   * 创建受管理的setInterval
   */
  setInterval(callback: () => void, delay: number): ReturnType<typeof setInterval> {
    const interval = setInterval(callback, delay)
    this.intervals.add(interval)
    return interval
  }

  /**
   * 注册观察者实例
   */
  addObserver<T extends IntersectionObserver | MutationObserver | ResizeObserver>(observer: T): T {
    this.observers.add(observer)
    return observer
  }

  /**
   * 注册事件监听器清理函数
   */
  addEventCleanup(cleanup: () => void): () => void {
    this.eventCleanups.add(cleanup)
    return cleanup
  }

  /**
   * 注册Image实例进行管理
   */
  addImageInstance(img: HTMLImageElement): HTMLImageElement {
    this.imageInstances.add(img)
    return img
  }

  /**
   * 清理单个定时器
   */
  clearTimeout(timer: ReturnType<typeof setTimeout>) {
    if (this.timers.has(timer)) {
      clearTimeout(timer)
      this.timers.delete(timer)
    }
  }

  /**
   * 清理单个间隔器
   */
  clearInterval(interval: ReturnType<typeof setInterval>) {
    if (this.intervals.has(interval)) {
      clearInterval(interval)
      this.intervals.delete(interval)
    }
  }

  /**
   * 手动清理单个Image实例
   */
  cleanupImage(img: HTMLImageElement) {
    if (this.imageInstances.has(img)) {
      img.onload = null
      img.onerror = null
      img.onabort = null
      img.src = ''
      this.imageInstances.delete(img)
    }
  }

  /**
   * 清理所有管理的资源
   */
  cleanup() {
    // 清理定时器
    this.timers.forEach(timer => {
      try {
        clearTimeout(timer)
      } catch (error) {
        console.warn('清理定时器失败:', error)
      }
    })
    this.timers.clear()

    // 清理间隔器
    this.intervals.forEach(interval => {
      try {
        clearInterval(interval)
      } catch (error) {
        console.warn('清理间隔器失败:', error)
      }
    })
    this.intervals.clear()

    // 清理观察者
    this.observers.forEach(observer => {
      try {
        observer.disconnect()
      } catch (error) {
        console.warn('清理观察者失败:', error)
      }
    })
    this.observers.clear()

    // 清理事件监听器
    this.eventCleanups.forEach(cleanup => {
      try {
        cleanup()
      } catch (error) {
        console.warn('清理事件监听器失败:', error)
      }
    })
    this.eventCleanups.clear()

    // 清理Image实例
    this.imageInstances.forEach(img => {
      try {
        img.onload = null
        img.onerror = null
        img.onabort = null
        img.src = ''
      } catch (error) {
        console.warn('清理Image实例失败:', error)
      }
    })
    this.imageInstances.clear()
  }

  /**
   * 获取当前管理的资源统计
   */
  getStats() {
    return {
      timers: this.timers.size,
      intervals: this.intervals.size,
      observers: this.observers.size,
      eventCleanups: this.eventCleanups.size,
      imageInstances: this.imageInstances.size
    }
  }
}

/**
 * Vue组合式函数 - 自动内存管理
 * 在组件卸载时自动清理所有注册的资源
 */
export function useMemoryManager() {
  const manager = new MemoryManager()

  // 组件卸载时自动清理
  onUnmounted(() => {
    manager.cleanup()
  })

  return manager
}

/**
 * 防抖函数生成器，支持取消操作
 */
export function createDebouncedFunction<T extends (...args: any[]) => any>(
  func: T,
  wait: number
): T & { cancel: () => void } {
  let timeout: ReturnType<typeof setTimeout> | null = null

  const debouncedFunc = function (...args: Parameters<T>) {
    if (timeout) clearTimeout(timeout)
    timeout = setTimeout(() => {
      timeout = null
      func(...args)
    }, wait)
  } as T

  // 添加取消方法
  ;(debouncedFunc as any).cancel = () => {
    if (timeout) {
      clearTimeout(timeout)
      timeout = null
    }
  }

  return debouncedFunc as T & { cancel: () => void }
}