import { jsonrepair } from 'jsonrepair';

// 轻量级highlight.js引入 - 包含所有需要的语言
import hljs from 'highlight.js/lib/core';
import javascript from 'highlight.js/lib/languages/javascript';
import typescript from 'highlight.js/lib/languages/typescript';
import python from 'highlight.js/lib/languages/python';
import sql from 'highlight.js/lib/languages/sql';
import xml from 'highlight.js/lib/languages/xml';
import json from 'highlight.js/lib/languages/json';

// 注册所有需要的语言
hljs.registerLanguage('javascript', javascript);
hljs.registerLanguage('typescript', typescript);
hljs.registerLanguage('python', python);
hljs.registerLanguage('sql', sql);
hljs.registerLanguage('xml', xml);
hljs.registerLanguage('html', xml); // HTML使用XML高亮
hljs.registerLanguage('json', json);

// 简化后的类型定义
export interface ContentType {
    type: 'json' | 'markdown' | 'code' | 'url' | 'email' | 'text';
    confidence: number; // 检测置信度 0-1
    originalType?: string; // 原始检测类型，用于语法高亮和格式化
}

export interface DetectedContent {
    original: string;
    formatted?: string;
    type: ContentType;
    preview: string; // 用于显示的预览文本
}

// 检测是否为JSON
function detectJSON(content: string): ContentType | null {
    const trimmed = content.trim();

    // 必须以 { 或 [ 开头，以 } 或 ] 结尾
    if (!(trimmed.startsWith('{') && trimmed.endsWith('}')) &&
        !(trimmed.startsWith('[') && trimmed.endsWith(']'))) {
        return null;
    }

    // 检查基本的JSON特征
    const jsonFeatures = [
        /"[^"]*"\s*:/, // 键值对
        /:\s*"[^"]*"/, // 字符串值
        /:\s*\d+/, // 数字值
        /:\s*(true|false|null)/, // 布尔值和null
        /\[\s*\{/, // 对象数组
        /\}\s*,\s*\{/ // 多个对象
    ];

    let featureCount = 0;
    for (const pattern of jsonFeatures) {
        if (pattern.test(trimmed)) {
            featureCount++;
        }
    }

    // 如果有足够的JSON特征，尝试解析（大内容只验证不解析）
    if (featureCount >= 2) {
        // 对于大内容（>50KB），跳过JSON.parse验证，直接基于特征判断
        if (trimmed.length > 50 * 1024) {
            return { type: 'json', confidence: 0.8 };
        }
        
        try {
            JSON.parse(trimmed);
            return { type: 'json', confidence: 0.95 };
        } catch {
            // 尝试修复JSON（小内容才做修复）
            if (trimmed.length <= 10 * 1024) {
                try {
                    jsonrepair(trimmed);
                    return { type: 'json', confidence: 0.8 };
                } catch {
                    return null;
                }
            }
            return null;
        }
    }

    return null;
}

// 统一的代码检测函数（合并SQL、HTML、XML检测）
function detectCodeType(content: string): ContentType | null {
    const trimmed = content.trim();
    
    // 对大内容只检测前20KB样本
    const sampleContent = trimmed.length > 50 * 1024 
        ? trimmed.substring(0, 20 * 1024)
        : trimmed;
    const upperContent = sampleContent.toUpperCase();

    // 检测SQL
    const sqlPatterns = [
        /^SELECT\s+.+\s+FROM\s+/i,
        /^INSERT\s+INTO\s+/i,
        /^UPDATE\s+.+\s+SET\s+/i,
        /^DELETE\s+FROM\s+/i,
        /^CREATE\s+(TABLE|DATABASE|INDEX|VIEW)\s+/i,
        /^ALTER\s+TABLE\s+/i,
        /^DROP\s+(TABLE|DATABASE|INDEX|VIEW)\s+/i
    ];

    const sqlKeywords = [
        'SELECT', 'FROM', 'WHERE', 'INSERT', 'UPDATE', 'DELETE',
        'CREATE', 'ALTER', 'DROP', 'JOIN', 'INNER', 'LEFT', 'RIGHT',
        'GROUP BY', 'ORDER BY', 'HAVING', 'LIMIT', 'DISTINCT'
    ];

    let sqlMatchCount = 0;
    let sqlKeywordCount = 0;

    for (const pattern of sqlPatterns) {
        if (pattern.test(sampleContent)) {
            sqlMatchCount++;
        }
    }

    for (const keyword of sqlKeywords) {
        if (upperContent.includes(keyword)) {
            sqlKeywordCount++;
        }
    }

    if (sqlMatchCount >= 1 || sqlKeywordCount >= 3) {
        const confidence = Math.min(0.9, (sqlMatchCount * 0.3) + (sqlKeywordCount * 0.1));
        return { type: 'code', confidence, originalType: 'sql' };
    }

    // 检测HTML
    const htmlFeatures = [
        /<!DOCTYPE\s+html/i,
        /<html[^>]*>/i,
        /<head[^>]*>/i,
        /<body[^>]*>/i,
        /<title[^>]*>/i
    ];

    const tagPattern = /<\/?[a-zA-Z][a-zA-Z0-9]*[^>]*>/g;
    const tags = sampleContent.match(tagPattern) || [];
    const tagDensity = tags.length / sampleContent.length;

    let htmlDocumentFeatures = 0;
    for (const pattern of htmlFeatures) {
        if (pattern.test(sampleContent)) {
            htmlDocumentFeatures++;
        }
    }

    const commonHtmlTags = ['div', 'span', 'p', 'a', 'img', 'ul', 'li', 'table', 'tr', 'td'];
    let commonTagCount = 0;
    for (const tag of commonHtmlTags) {
        if (new RegExp(`<\\/?${tag}[^>]*>`, 'i').test(sampleContent)) {
            commonTagCount++;
        }
    }

    if (htmlDocumentFeatures >= 2 || tagDensity > 0.05 || (tags.length >= 3 && commonTagCount >= 2)) {
        const confidence = Math.min(0.9, htmlDocumentFeatures * 0.2 + tagDensity * 10 + commonTagCount * 0.1);
        return { type: 'code', confidence, originalType: 'html' };
    }

    // 检测XML
    const hasXmlDecl = /^<\?xml\s+version\s*=\s*["'][^"']*["'][^>]*\?>/i.test(sampleContent);
    const xmlFeatures = [
        /^<\?xml/i,                     // XML声明
        /<\/[a-zA-Z][a-zA-Z0-9:]*>/,   // 结束标签
        /xmlns[^=]*=/i,                 // 命名空间
        /<[a-zA-Z][a-zA-Z0-9:]*[^>]*\/>/,  // 自闭合标签
        /<!\[CDATA\[/,                  // CDATA
        /<!--[\s\S]*?-->/               // XML注释
    ];

    let xmlFeatureCount = 0;
    for (const pattern of xmlFeatures) {
        if (pattern.test(sampleContent)) {
            xmlFeatureCount++;
        }
    }

    const openTags = (sampleContent.match(/<[a-zA-Z][a-zA-Z0-9:]*[^>\/]*>/g) || []).length;
    const closeTags = (sampleContent.match(/<\/[a-zA-Z][a-zA-Z0-9:]*>/g) || []).length;
    const hasBalancedTags = Math.abs(openTags - closeTags) <= 1;
    const hasXmlStructure = hasXmlDecl || xmlFeatureCount >= 2 || (tagDensity > 0.05 && hasBalancedTags);

    if (hasXmlStructure && tags.length >= 2) {
        let confidence = 0.6;
        if (hasXmlDecl) confidence += 0.3;
        if (xmlFeatureCount >= 2) confidence += 0.2;
        if (hasBalancedTags) confidence += 0.1;

        return { type: 'code', confidence: Math.min(0.9, confidence), originalType: 'xml' };
    }

    return null;
}

// 检测URL
function detectURL(content: string): ContentType | null {
    const urlPattern = /^https?:\/\/[^\s]+$/i;
    const multiUrlPattern = /https?:\/\/[^\s]+/gi;

    const trimmed = content.trim();

    // 单个URL
    if (urlPattern.test(trimmed)) {
        return { type: 'url', confidence: 0.95 };
    }

    // 多个URL（每行一个或空格分隔）
    const urls = trimmed.match(multiUrlPattern) || [];
    const lines = trimmed.split('\n').filter(line => line.trim());

    if (urls.length >= 2 && urls.length >= lines.length * 0.7) {
        return { type: 'url', confidence: 0.85 };
    }

    return null;
}

// 检测邮箱
function detectEmail(content: string): ContentType | null {
    const emailPattern = /^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$/;
    const multiEmailPattern = /[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}/g;

    const trimmed = content.trim();

    // 单个邮箱
    if (emailPattern.test(trimmed)) {
        return { type: 'email', confidence: 0.95 };
    }

    // 多个邮箱
    const emails = trimmed.match(multiEmailPattern) || [];
    const lines = trimmed.split('\n').filter(line => line.trim());

    if (emails.length >= 2 && emails.length >= lines.length * 0.7) {
        return { type: 'email', confidence: 0.85 };
    }

    return null;
}

// 检测Markdown
function detectMarkdown(content: string): ContentType | null {
    const markdownFeatures = [
        /^#{1,6}\s+.+$/gm,              // 标题
        /\*\*[^*]+\*\*/,                // 粗体
        /\*[^*]+\*/,                    // 斜体
        /`[^`]+`/,                      // 行内代码
        /```[\s\S]*?```/,               // 代码块
        /^\s*[\*\-\+]\s+/gm,           // 无序列表
        /^\s*\d+\.\s+/gm,              // 有序列表
        /^\s*>\s+/gm,                   // 引用
        /\[([^\]]+)\]\(([^)]+)\)/,     // 链接
        /!\[([^\]]*)\]\(([^)]+)\)/,    // 图片
        /^\s*\|.*\|.*$/gm,             // 表格
        /^\s*[-]{3,}\s*$/gm,           // 分割线
    ];

    let featureCount = 0;
    for (const pattern of markdownFeatures) {
        if (pattern.test(content)) {
            featureCount++;
        }
    }

    // 检查特殊的Markdown文档特征
    const hasHeaders = /^#{1,6}\s+/gm.test(content);
    const hasCodeBlocks = /```/.test(content);
    const hasLinks = /\[.*\]\(.*\)/.test(content);
    const hasList = /^\s*[\*\-\+\d\.]\s+/gm.test(content);

    // 如果有多个Markdown特征，认为是Markdown
    if (featureCount >= 3 || (hasHeaders && (hasCodeBlocks || hasLinks || hasList))) {
        const confidence = Math.min(0.85, featureCount * 0.15 + (hasHeaders ? 0.2 : 0));
        return { type: 'markdown', confidence };
    }

    return null;
}

// 通用代码检测（用于检测JavaScript、Python等）
function detectGeneralCode(content: string): ContentType | null {
    // 最小长度要求
    if (!content || content.length < 10) return null;

    const lines = content.split('\n');
    const isVeryLongText = content.length > 1000 && lines.length > 20;

    try {
        // 对于大于50KB的内容，只取前10KB进行hljs检测，避免性能问题
        const sampleContent = content.length > 50 * 1024 
            ? content.substring(0, 10 * 1024)
            : content;
            
        // 使用 highlight.js 自动检测语言（仅检测样本）
        const { language, relevance } = hljs.highlightAuto(sampleContent);

        // 1. highlight.js 检测成功 + 置信度较高
        if (language && relevance >= 5) {
            const minRelevance = isVeryLongText ? 8 : 5;
            if (relevance >= minRelevance) {
                return {
                    type: 'code',
                    confidence: Math.min(0.9, relevance / 10),
                    originalType: language
                };
            }
        }

        // 2. 启发式检测
        const keywords = [
            'function', 'const', 'let', 'var', 'return', 'if', 'else', 'while',
            'for', 'switch', 'class', 'import', 'export', 'def', 'fn', '#include',
            'public', 'private', '=>', '===', '!==', 'new', 'try', 'catch',
            'interface', 'extends', 'implements', 'async', 'await', 'typeof'
        ];

        // 对大内容只检测前10KB的样本，避免正则表达式性能问题
        const detectionSample = content.length > 50 * 1024 
            ? content.substring(0, 10 * 1024)
            : content;
            
        const score = keywords.reduce((acc, kw) => acc + (detectionSample.includes(kw) ? 1 : 0), 0);
        const indentedLines = lines.slice(0, Math.min(lines.length, 100)).filter(line => line.startsWith('  ') || line.startsWith('\t')).length;
        const hasSymbols = /[{};=()\[\]]/.test(detectionSample);
        const hasFunctionCalls = /\w+\s*\(/.test(detectionSample);
        const hasCodeComments = /\/\/|\/\*|\*\/|#(?!\s*\w+\s*$)|<!--/.test(detectionSample);

        const symbolMatches = detectionSample.match(/[{};=()\[\]]/g) || [];
        const symbolDensity = symbolMatches.length / detectionSample.length;
        const hasNaturalLanguage = /[.!?]+\s+[A-Z]/.test(detectionSample);
        const hasContinuousText = /\w{15,}/.test(detectionSample);
        const hasCommonWords = /\b(the|and|or|but|in|on|at|to|for|of|with|by)\b/gi.test(detectionSample);

        const isLikelyCode = (
            (score >= 2 && hasSymbols && hasFunctionCalls) ||
            (indentedLines >= 2 && lines.length >= 3 && symbolDensity > 0.01) ||
            (hasCodeComments && (score >= 1 || symbolDensity > 0.005))
        );

        const isLikelyNaturalText = hasNaturalLanguage && hasContinuousText && hasCommonWords && symbolDensity < 0.005;

        if (isLikelyCode && !isLikelyNaturalText) {
            const baseConfidence = isVeryLongText ? 0.6 : 0.75;
            const hljsBonus = (language && relevance >= 3) ? 0.15 : 0;
            return {
                type: 'code',
                confidence: Math.min(0.85, baseConfidence + hljsBonus),
                originalType: language || 'javascript'
            };
        }

        // 3. 低置信度但有明确代码特征的情况
        if (language && relevance >= 2) {
            const codePatterns = [
                /function\s+\w+\s*\(/,
                /(const|let|var)\s+\w+\s*=/,
                /\bif\s*\([^)]+\)\s*\{/,
                /import\s+.+\s+from/,
                /class\s+\w+/,
                /\w+\.\w+\(/
            ];

            const patternMatches = codePatterns.reduce((count, pattern) =>
                pattern.test(content) ? count + 1 : count, 0);

            if (patternMatches >= 2) {
                return {
                    type: 'code',
                    confidence: Math.min(0.8, 0.5 + (patternMatches * 0.1) + (relevance / 20)),
                    originalType: language
                };
            }
        }

    } catch (error) {
        console.warn('highlight.js 检测失败:', error);
    }

    return null;
}

// 主检测函数（优化检测顺序和早期退出）
export function detectContentType(content: string): DetectedContent {
    if (!content || content.trim().length === 0) {
        return {
            original: content,
            type: { type: 'text', confidence: 1 },
            preview: content
        };
    }

    // 对于超大文本（>500KB），简化检测避免性能问题
    if (content.length > 500 * 1024) {
        return {
            original: content,
            type: { type: 'text', confidence: 1 },
            preview: content.substring(0, 200) + (content.length > 200 ? '...' : '')
        };
    }

    // 快速预筛选：根据内容特征选择性运行检测器
    const trimmed = content.trim();
    const detectors: Array<() => ContentType | null> = [];

    // JSON检测 - 高优先级，快速判断
    if ((trimmed.startsWith('{') && trimmed.endsWith('}')) ||
        (trimmed.startsWith('[') && trimmed.endsWith(']'))) {
        detectors.push(() => detectJSON(content));
    }

    // URL/Email检测 - 快速模式匹配
    if (/^https?:\/\/|@.*\./i.test(trimmed)) {
        detectors.push(() => detectURL(content));
        detectors.push(() => detectEmail(content));
    }

    // Markdown检测 - 特征明显时优先检测
    if (/^#+\s|```|\*\*|\[[^\]]+\]\(/m.test(content)) {
        detectors.push(() => detectMarkdown(content));
    }

    // 代码检测 - 包括 SQL、HTML、XML 和通用代码
    detectors.push(() => detectCodeType(content));
    detectors.push(() => detectGeneralCode(content));

    // 运行检测器，找到高置信度结果即停止
    for (const detector of detectors) {
        const result = detector();
        if (result && result.confidence > 0.8) {
            return {
                original: content,
                type: result,
                preview: content.substring(0, 200) + (content.length > 200 ? '...' : '')
            };
        }
    }

    // 如果没有高置信度结果，运行所有检测器并选择最佳结果
    const allResults = detectors
        .map(detector => detector())
        .filter(result => result !== null)
        .sort((a, b) => b!.confidence - a!.confidence);

    const bestResult = allResults.length > 0 && allResults[0]!.confidence > 0.5
        ? allResults[0]!
        : { type: 'text', confidence: 1 } as ContentType;

    return {
        original: content,
        type: bestResult,
        preview: content.substring(0, 200) + (content.length > 200 ? '...' : '')
    };
}

// 格式化内容
export function formatContent(content: string, type: ContentType): string {
    try {
        switch (type.type) {
            case 'json':
                try {
                    const parsed = JSON.parse(content);
                    return JSON.stringify(parsed, null, 2);
                } catch {
                    try {
                        const repaired = jsonrepair(content);
                        const parsed = JSON.parse(repaired);
                        return JSON.stringify(parsed, null, 2);
                    } catch {
                        return content;
                    }
                }
            case 'code':
                // SQL格式化
                if (type.originalType === 'sql') {
                    return content
                        .replace(/\bSELECT\b/gi, 'SELECT')
                        .replace(/\bFROM\b/gi, '\nFROM')
                        .replace(/\bWHERE\b/gi, '\nWHERE')
                        .replace(/\bAND\b/gi, '\n  AND')
                        .replace(/\bOR\b/gi, '\n  OR')
                        .replace(/\bJOIN\b/gi, '\nJOIN')
                        .replace(/\bGROUP BY\b/gi, '\nGROUP BY')
                        .replace(/\bORDER BY\b/gi, '\nORDER BY')
                        .replace(/\bLIMIT\b/gi, '\nLIMIT');
                }
                return content;
            default:
                return content;
        }
    } catch {
        return content;
    }
}

// 获取语法高亮的语言类型
export function getHighlightLanguage(_content: string, type: ContentType): string {
    switch (type.type) {
        case 'json':
            return 'json';
        case 'markdown':
            return 'markdown';
        case 'code':
            return type.originalType || 'javascript';
        default:
            return 'javascript';
    }
}

// 导出独立的代码检测函数
export function isCodeSnippet(text: string): boolean {
    const codeResult = detectCodeType(text) || detectGeneralCode(text);
    return codeResult !== null && codeResult.confidence > 0.5;
}

// 导出带详细信息的代码检测函数
export function detectCodeWithDetails(text: string): { isCode: boolean; confidence: number; language?: string; relevance?: number } {
    if (!text || text.length < 10) {
        return { isCode: false, confidence: 0 };
    }

    try {
        const { language, relevance } = hljs.highlightAuto(text);
        const codeResult = detectCodeType(text) || detectGeneralCode(text);

        return {
            isCode: codeResult !== null && codeResult.confidence > 0.5,
            confidence: codeResult?.confidence || 0,
            language: codeResult?.originalType || language || undefined,
            relevance: relevance || 0
        };
    } catch (error) {
        console.warn('代码检测失败:', error);
        return { isCode: false, confidence: 0 };
    }
} 