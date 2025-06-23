use std::collections::HashSet;

use charabia::Tokenize;

/// 对一个str进行默认分词   返回去重后所有分词结果
pub async fn tokenize_str(str: &str) -> HashSet<String> {
    let t_res = str.tokenize();
    let mut res = HashSet::new();
    for i in t_res {
        if i.kind() == charabia::TokenKind::Word {
            res.insert(i.lemma().to_string());
        }
    }
    res
}
