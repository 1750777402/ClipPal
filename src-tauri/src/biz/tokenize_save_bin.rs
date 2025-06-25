use crate::utils::tokenize_util::tokenize_str;

///  对content进行分词并保存到bin文件中
pub async fn content_tokenize_save_bin(id: &str, content: &str) {
    let t = tokenize_str(content).await;
    println!("分词后的所有词: {:?}", t);
}
