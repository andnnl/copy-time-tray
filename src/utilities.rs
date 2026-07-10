use chrono::Local;
use md5::{Digest, Md5};
use rand::Rng;

pub fn generate_password(length: usize) -> String {
    const CHARSET: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*";
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

pub fn generate_md5(input: &str, length: usize) -> String {
    let mut hasher = Md5::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    let hex = format!("{:x}", result);
    match length {
        16 => hex[8..24].to_string(),
        _ => hex,
    }
}

pub fn get_time_text(format: &str) -> String {
    let now = Local::now();
    match format {
        "date" => now.format("%Y-%m-%d").to_string(),
        "compact" => now.format("%Y%m%d%H%M%S").to_string(),
        _ => now.format("%Y-%m-%d %H:%M:%S").to_string(),
    }
}
