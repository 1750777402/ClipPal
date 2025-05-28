use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClipType {
    Text,
    Img,
    File,
    Rtf,
    Html,
    Unknown,
}
