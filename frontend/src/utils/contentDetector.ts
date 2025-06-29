import { jsonrepair } from 'jsonrepair';

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
        return { type: 'sql', confidence };
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
        return { type: 'html', confidence };
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

        return { type: 'xml', confidence: Math.min(0.9, confidence) };
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

// 检测代码（通用代码特征检测，包含前端、后端、脚本等）
function detectCode(content: string): ContentType | null {
    const codeFeatures = [
        // 函数定义（多语言）
        /function\s+\w+\s*\(/,              // JavaScript/TypeScript
        /def\s+\w+\s*\(/,                   // Python
        /(public|private|protected)\s+(static\s+)?\w+\s+\w+\s*\(/,  // Java/C#
        /func\s+\w+\s*\(/,                  // Go/Swift
        /fn\s+\w+\s*\(/,                    // Rust

        // 变量声明
        /(const|let|var)\s+\w+\s*=/,       // JavaScript/TypeScript
        /\w+\s*=\s*new\s+\w+/,             // 对象实例化
        /\$\w+\s*=/,                       // PHP/Shell变量

        // 控制结构
        /\bif\s*\([^)]+\)\s*\{/,
        /\bfor\s*\([^)]*\)\s*\{/,
        /\bwhile\s*\([^)]+\)\s*\{/,
        /\btry\s*\{[\s\S]*\}\s*catch/,
        /\bswitch\s*\([^)]+\)\s*\{/,

        // 导入和引用
        /import\s+.+\s+from\s+/,           // ES6 imports
        /#include\s*<.+>/,                 // C/C++
        /using\s+\w+/,                     // C#
        /require\s*\(['"]/,                // Node.js
        /from\s+\w+\s+import/,             // Python

        // 前端框架特征
        /<template[^>]*>/i,                // Vue
        /\{\{[^}]+\}\}/,                   // Vue/Angular插值
        /v-[a-z]+=/i,                      // Vue指令
        /className\s*=/,                   // React JSX
        /useState|useEffect/,              // React Hooks
        /@Component/i,                     // Angular

        // 代码块和结构特征
        /\{\s*[\r\n]/,                     // 代码块开始
        /;\s*[\r\n]/,                      // 语句结束
        /\/\/.*$/gm,                       // 单行注释
        /\/\*[\s\S]*?\*\//,               // 多行注释
        /#.*$/gm,                          // Python/Shell注释
        /--.*$/gm,                         // SQL注释

        // 其他编程语言特征
        /\bclass\s+\w+/,                   // 类定义
        /\binterface\s+\w+/,               // 接口定义
        /\w+\.\w+\(/,                      // 方法调用
        /=>\s*\{/,                         // 箭头函数
        /\[\w+\]/,                         // 数组访问或属性
    ];

    let featureCount = 0;
    for (const pattern of codeFeatures) {
        if (pattern.test(content)) {
            featureCount++;
        }
    }

    // 计算代码密度指标
    const lines = content.split('\n');

    // 检查是否有典型的代码结构
    const hasCodeBlocks = /\{[\s\S]*\}/.test(content);
    const hasSemicolons = (content.match(/;/g) || []).length >= 2;
    const hasIndentation = lines.some(line => /^\s{2,}/.test(line));
    const hasOperators = /[+\-*\/=<>!&|]{1,2}/.test(content);

    // 提高检测精度，降低误判
    const codeScore = featureCount * 0.15 +
        (hasCodeBlocks ? 0.25 : 0) +
        (hasIndentation ? 0.15 : 0) +
        (hasSemicolons ? 0.1 : 0) +
        (hasOperators ? 0.05 : 0);

    if (featureCount >= 2 || codeScore >= 0.5) {
        const confidence = Math.min(0.85, codeScore);
        return { type: 'code', confidence };
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
            case 'sql':
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