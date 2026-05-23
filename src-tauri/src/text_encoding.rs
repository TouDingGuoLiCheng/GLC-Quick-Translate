/// 将字节解码为 UTF-8 文本；Windows 下对控制台/剪贴板常见的 GBK 做回退。
pub fn decode_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.trim().to_string();
    }
    #[cfg(windows)]
    {
        let (cow, _, _) = encoding_rs::GBK.decode(bytes);
        let s = cow.into_owned();
        if !s.is_empty() {
            return s.trim().to_string();
        }
    }
    String::from_utf8_lossy(bytes).trim().to_string()
}
