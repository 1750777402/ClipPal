use clipboard_listener::{ClipBoardEventListener, ClipType, ClipboardEvent};

#[derive(Debug, Clone)]
pub struct ClipboardEventTigger;
#[async_trait::async_trait]
impl ClipBoardEventListener<ClipboardEvent> for ClipboardEventTigger {
    async fn handle_event(&self, event: &ClipboardEvent) {
        println!("触发了粘贴板监听器，内容：{:?}", &event.r#type);
        match event.r#type {
            ClipType::Text => {
                println!("文本内容：{}", event.content);
            }
            ClipType::Img => {
                println!("图片内容：{}", event.content);
            }
            ClipType::File => {
                println!("文件内容：{}", event.content);
            }
            _ => {}
        }
    }
}
