import { jsonrepair } from 'jsonrepair';

// 轻量级highlight.js引入 - 只引入核心部分
import hljs from 'highlight.js/lib/core';
import javascript from 'highlight.js/lib/languages/javascript';
import typescript from 'highlight.js/lib/languages/typescript';
import python from 'highlight.js/lib/languages/python';

// 注册必要的语言
hljs.registerLanguage('javascript', javascript);
hljs.registerLanguage('typescript', typescript);
hljs.registerLanguage('python', python);

export interface ContentType {
    type: 'json' | 'xml' | 'sql' | 'html' | 'markdown' | 'code' | 'url' | 'email' | 'text';
    confidence: number; // 检测置信度 0-1
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

    // 如果有足够的JSON特征，尝试解析
    if (featureCount >= 2) {
        try {
            JSON.parse(trimmed);
            return { type: 'json', confidence: 0.95 };
        } catch {
            // 尝试修复JSON
            try {
                jsonrepair(trimmed);
                return { type: 'json', confidence: 0.8 };
            } catch {
                return null;
            }
        }
    }

    return null;
}

// 检测SQL语句
function detectSQL(content: string): ContentType | null {
    const upperContent = content.toUpperCase().trim();

    // SQL关键字模式
    const sqlPatterns = [
        /^SELECT\s+.+\s+FROM\s+/i,
        /^INSERT\s+INTO\s+/i,
        /^UPDATE\s+.+\s+SET\s+/i,
        /^DELETE\s+FROM\s+/i,
        /^CREATE\s+(TABLE|DATABASE|INDEX|VIEW)\s+/i,
        /^ALTER\s+TABLE\s+/i,
        /^DROP\s+(TABLE|DATABASE|INDEX|VIEW)\s+/i,
        /\s+WHERE\s+.+/i,
        /\s+JOIN\s+.+\s+ON\s+/i,
        /\s+GROUP\s+BY\s+/i,
        /\s+ORDER\s+BY\s+/i
    ];

    let matchCount = 0;
    for (const pattern of sqlPatterns) {
        if (pattern.test(content)) {
            matchCount++;
        }
    }

    // SQL特有的符号和结构
    const sqlKeywords = [
        'SELECT', 'FROM', 'WHERE', 'INSERT', 'UPDATE', 'DELETE',
        'CREATE', 'ALTER', 'DROP', 'JOIN', 'INNER', 'LEFT', 'RIGHT',
        'GROUP BY', 'ORDER BY', 'HAVING', 'LIMIT', 'DISTINCT'
    ];

    let keywordCount = 0;
    for (const keyword of sqlKeywords) {
        if (upperContent.includes(keyword)) {
            keywordCount++;
        }
    }

    if (matchCount >= 1 || keywordCount >= 3) {
        const confidence = Math.min(0.9, (matchCount * 0.3) + (keywordCount * 0.1));
        return { type: 'code', confidence };
    }

    return null;
}

// 检测HTML
function detectHTML(content: string): ContentType | null {
    const trimmed = content.trim();

    // HTML文档特征
    const htmlFeatures = [
        /<!DOCTYPE\s+html/i,
        /<html[^>]*>/i,
        /<head[^>]*>/i,
        /<body[^>]*>/i,
        /<title[^>]*>/i
    ];

    // HTML标签模式
    const tagPattern = /<\/?[a-zA-Z][a-zA-Z0-9]*[^>]*>/g;
    const tags = trimmed.match(tagPattern) || [];

    // 计算标签密度
    const tagDensity = tags.length / trimmed.length;

    // 检查HTML文档特征
    let documentFeatures = 0;
    for (const pattern of htmlFeatures) {
        if (pattern.test(trimmed)) {
            documentFeatures++;
        }
    }

    // 常见HTML标签
    const commonTags = ['div', 'span', 'p', 'a', 'img', 'ul', 'li', 'table', 'tr', 'td'];
    let commonTagCount = 0;
    for (const tag of commonTags) {
        if (new RegExp(`<\\/?${tag}[^>]*>`, 'i').test(trimmed)) {
            commonTagCount++;
        }
    }

    // 判断是否为HTML
    if (documentFeatures >= 2 || tagDensity > 0.05 || (tags.length >= 3 && commonTagCount >= 2)) {
        const confidence = Math.min(0.9, documentFeatures * 0.2 + tagDensity * 10 + commonTagCount * 0.1);
        return { type: 'code', confidence };
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

// 检测XML文档
function detectXML(content: string): ContentType | null {
    const trimmed = content.trim();

    // XML声明
    const hasXmlDecl = /^<\?xml\s+version\s*=\s*["'][^"']*["'][^>]*\?>/i.test(trimmed);

    // XML标签模式
    const tagPattern = /<\/?[a-zA-Z][a-zA-Z0-9:]*[^>]*>/g;
    const tags = trimmed.match(tagPattern) || [];

    // 检查是否有XML特征
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
        if (pattern.test(trimmed)) {
            xmlFeatureCount++;
        }
    }

    // 计算标签密度
    const tagDensity = tags.length / trimmed.length;

    // 检查是否有闭合标签匹配
    const openTags = (trimmed.match(/<[a-zA-Z][a-zA-Z0-9:]*[^>\/]*>/g) || []).length;
    const closeTags = (trimmed.match(/<\/[a-zA-Z][a-zA-Z0-9:]*>/g) || []).length;

    // 判断是否为XML
    const hasBalancedTags = Math.abs(openTags - closeTags) <= 1;
    const hasXmlStructure = hasXmlDecl || xmlFeatureCount >= 2 || (tagDensity > 0.05 && hasBalancedTags);

    if (hasXmlStructure && tags.length >= 2) {
        let confidence = 0.6;
        if (hasXmlDecl) confidence += 0.3;
        if (xmlFeatureCount >= 2) confidence += 0.2;
        if (hasBalancedTags) confidence += 0.1;

        return { type: 'code', confidence: Math.min(0.9, confidence) };
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

// 自定义启发式检测规则
function heuristicCodeCheck(text: string): boolean {
    const keywords = [
        'function', 'const', 'let', 'var', 'return', 'if', 'else', 'while',
        'for', 'switch', 'class', 'import', 'export', 'def', 'fn', '#include',
        'public', 'private', '=>', '===', '!==', 'new', 'try', 'catch',
        'interface', 'extends', 'implements', 'async', 'await', 'typeof'
    ];

    const score = keywords.reduce((acc, kw) => acc + (text.includes(kw) ? 1 : 0), 0);

    const lines = text.split('\n');
    const indentedLines = lines.filter(line =>
        line.startsWith('  ') || line.startsWith('\t')
    ).length;

    const hasMultipleLines = lines.length >= 3;
    const hasSymbols = /[{};=()\[\]]/.test(text);
    const hasFunctionCalls = /\w+\s*\(/.test(text);
    const hasCodeComments = /\/\/|\/\*|\*\/|#(?!\s*\w+\s*$)|<!--/.test(text);

    // 计算代码特征密度
    const textLength = text.length;
    const symbolDensity = (text.match(/[{};=()\[\]]/g) || []).length / textLength;

    // 检查是否为自然语言文本特征
    const hasNaturalLanguage = /[.!?]+\s+[A-Z]/.test(text); // 句子结构
    const hasContinuousText = /\w{15,}/.test(text); // 长单词（可能是自然语言）
    const hasCommonWords = /\b(the|and|or|but|in|on|at|to|for|of|with|by)\b/gi.test(text);

    // 强化判断逻辑
    const isLikelyCode = (
        (score >= 2 && hasSymbols && hasFunctionCalls) ||
        (indentedLines >= 2 && hasMultipleLines && symbolDensity > 0.01) ||
        (hasCodeComments && (score >= 1 || symbolDensity > 0.005))
    );

    // 排除自然语言特征明显的文本
    const isLikelyNaturalText = hasNaturalLanguage && hasContinuousText && hasCommonWords && symbolDensity < 0.005;

    return isLikelyCode && !isLikelyNaturalText;
}

// 检测代码（结合highlight.js和启发式判断）
function detectCode(content: string): ContentType | null {
    // 最小长度要求，过短的内容不太可能是有意义的代码
    if (!content || content.length < 10) return null;

    // 如果文本过长且没有明显代码结构，降低识别为代码的可能性
    const lines = content.split('\n');
    const isVeryLongText = content.length > 1000 && lines.length > 20;

    try {
        // 使用 highlight.js 自动检测语言（仅限注册的语言）
        const { language, relevance } = hljs.highlightAuto(content);

        // 1. highlight.js 检测成功 + 置信度较高
        if (language && relevance >= 5) {
            // 对于长文本，提高阈值要求
            const minRelevance = isVeryLongText ? 8 : 5;
            if (relevance >= minRelevance) {
                return { type: 'code', confidence: Math.min(0.9, relevance / 10) };
            }
        }

        // 2. 自定义启发式判断兜底
        const heuristicResult = heuristicCodeCheck(content);
        if (heuristicResult) {
            // 对于长文本，降低置信度
            const baseConfidence = isVeryLongText ? 0.6 : 0.75;
            const hljsBonus = (language && relevance >= 3) ? 0.15 : 0;
            return {
                type: 'code',
                confidence: Math.min(0.85, baseConfidence + hljsBonus)
            };
        }

        // 3. 如果highlight.js检测到语言但置信度不够高，进行二次验证
        if (language && relevance >= 2) {
            // 检查一些明确的代码特征
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
                const confidence = Math.min(0.8, 0.5 + (patternMatches * 0.1) + (relevance / 20));
                return { type: 'code', confidence };
            }
        }

    } catch (error) {
        console.warn('highlight.js 检测失败，使用启发式检测:', error);
        // 降级到启发式检测
        const heuristicResult = heuristicCodeCheck(content);
        if (heuristicResult) {
            return { type: 'code', confidence: 0.6 };
        }
    }

    return null;
}

// 主检测函数
export function detectContentType(content: string): DetectedContent {
    if (!content || content.trim().length === 0) {
        return {
            original: content,
            type: { type: 'text', confidence: 1 },
            preview: content
        };
    }

    const detectors = [
        detectJSON,          // 最高优先级
        detectXML,           // XML文档
        detectSQL,           // SQL查询  
        detectHTML,          // HTML文档
        detectMarkdown,      // Markdown文档
        detectURL,           // URL链接
        detectEmail,         // 邮箱地址
        detectCode           // 通用代码（最低优先级）
    ];

    // 运行所有检测器
    const results = detectors
        .map(detector => detector(content))
        .filter(result => result !== null)
        .sort((a, b) => b!.confidence - a!.confidence);

    // 选择置信度最高的结果，如果置信度太低就归类为文本
    const bestResult = results.length > 0 && results[0]!.confidence > 0.5
        ? results[0]!
        : { type: 'text', confidence: 1 } as ContentType;

    return {
        original: content,
        type: bestResult,
        preview: content.substring(0, 200) + (content.length > 200 ? '...' : '')
    };
}

// 检测内容的原始类型（用于格式化和语法高亮）
function detectOriginalType(content: string): string {
    // 检测SQL
    const upperContent = content.toUpperCase().trim();
    const sqlPatterns = [
        /^SELECT\s+.+\s+FROM\s+/i,
        /^INSERT\s+INTO\s+/i,
        /^UPDATE\s+.+\s+SET\s+/i,
        /^DELETE\s+FROM\s+/i,
        /^CREATE\s+(TABLE|DATABASE|INDEX|VIEW)\s+/i
    ];
    const sqlKeywords = ['SELECT', 'FROM', 'WHERE', 'INSERT', 'UPDATE', 'DELETE', 'CREATE', 'ALTER', 'DROP'];

    if (sqlPatterns.some(pattern => pattern.test(content)) ||
        sqlKeywords.filter(kw => upperContent.includes(kw)).length >= 3) {
        return 'sql';
    }

    // 检测HTML
    const trimmed = content.trim();
    const htmlFeatures = [
        /<!DOCTYPE\s+html/i,
        /<html[^>]*>/i,
        /<head[^>]*>/i,
        /<body[^>]*>/i,
        /<title[^>]*>/i
    ];
    const tagPattern = /<\/?[a-zA-Z][a-zA-Z0-9]*[^>]*>/g;
    const tags = trimmed.match(tagPattern) || [];

    if (htmlFeatures.some(pattern => pattern.test(trimmed)) || tags.length >= 3) {
        return 'html';
    }

    // 检测XML
    const hasXmlDecl = /^<\?xml\s+version\s*=\s*["'][^"']*["'][^>]*\?>/i.test(trimmed);
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
        if (pattern.test(trimmed)) {
            xmlFeatureCount++;
        }
    }

    if (hasXmlDecl || xmlFeatureCount >= 2) {
        return 'xml';
    }

    return 'code';
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
                    // 尝试修复JSON
                    try {
                        const repaired = jsonrepair(content);
                        const parsed = JSON.parse(repaired);
                        return JSON.stringify(parsed, null, 2);
                    } catch {
                        return content;
                    }
                }
            case 'code':
                // 对于统一归类为code的内容，检测原始类型进行格式化
                const originalType = detectOriginalType(content);
                if (originalType === 'sql') {
                    // 简单的SQL格式化：添加换行和缩进
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
            case 'xml':
                // XML保持原始格式，由语法高亮处理
                return content;
            case 'markdown':
                // Markdown保持原始格式，让渲染器处理
                return content;
            default:
                return content;
        }
    } catch {
        return content;
    }
}

// 获取语法高亮的语言类型
export function getHighlightLanguage(content: string, type: ContentType): string {
    switch (type.type) {
        case 'json':
            return 'json';
        case 'markdown':
            return 'markdown';
        case 'code':
            // 对于统一归类为code的内容，检测原始类型
            const originalType = detectOriginalType(content);
            if (originalType === 'sql') return 'sql';
            if (originalType === 'html') return 'html';
            if (originalType === 'xml') return 'xml';
            return 'javascript'; // 默认使用JavaScript高亮
        default:
            return 'javascript';
    }
}

// 导出独立的代码检测函数（优化版本）
export function isCodeSnippet(text: string): boolean {
    const result = detectCode(text);
    return result !== null && result.confidence > 0.5;
}

// 导出带详细信息的代码检测函数
export function detectCodeWithDetails(text: string): { isCode: boolean; confidence: number; language?: string; relevance?: number } {
    if (!text || text.length < 10) {
        return { isCode: false, confidence: 0 };
    }

    try {
        // 使用 highlight.js 获取详细信息
        const { language, relevance } = hljs.highlightAuto(text);
        const codeResult = detectCode(text);

        return {
            isCode: codeResult !== null && codeResult.confidence > 0.5,
            confidence: codeResult?.confidence || 0,
            language: language || undefined,
            relevance: relevance || 0
        };
    } catch (error) {
        console.warn('代码检测失败:', error);
        const codeResult = detectCode(text);
        return {
            isCode: codeResult !== null && codeResult.confidence > 0.5,
            confidence: codeResult?.confidence || 0,
            language: undefined,
            relevance: 0
        };
    }
} 