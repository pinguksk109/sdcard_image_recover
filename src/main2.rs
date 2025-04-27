// バッファ全体を1バイトずつ走査するパターン
use std::fs::{File, create_dir_all};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Instant;

const DEVICE_PATH: &str = "/dev/mmcblk0";
const OUTPUT_DIR: &str = "recovered";
const JPEG_START: &[u8] = &[0xFF, 0xD8];
const RW2_START: &[u8] = &[0x49, 0x49, 0x2A, 0x00];
const READ_BLOCK_SIZE: usize = 32 * 1024 * 1024;

#[derive(Debug, PartialEq)]
enum FileType {
    Jpeg,
    Rw2,
}

fn main() {
    if !Path::new(OUTPUT_DIR).exists() {
        if let Err(e) = create_dir_all(OUTPUT_DIR) {
            eprintln!("保存先ディレクトリの作成に失敗しました: {}", e);
            return;
        }
    }

    let start_time = Instant::now();
    let mut counter = 0;

    let mut file = match File::open(DEVICE_PATH) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("デバイスを開けませんでした: {}", e);
            return;
        }
    };

    let mut buffer = Vec::new();
    let mut temp = vec![0u8; READ_BLOCK_SIZE];

    println!("データ読み込み中...");
    loop {
        match file.read(&mut temp) {
            Ok(0) => break,
            Ok(n) => buffer.extend_from_slice(&temp[..n]),
            Err(e) => {
                eprintln!("読み取りエラー: {}", e);
                break;
            }
        }
    }
    println!("読み込み完了、スキャン開始！");

    let mut pos = 0;
    while pos < buffer.len() {
        if let Some(file_type) = match_start(&buffer[pos..]) {
            let start = pos;
            let end = match find_next_start(&buffer[pos + 1..]) {
                Some(next_start) => pos + 1 + next_start,
                None => buffer.len(),
            };

            save_file(&buffer[start..end], counter, file_type);
            counter += 1;
            pos = end;
        } else {
            pos += 1;
        }
    }

    let duration = start_time.elapsed();
    println!("\n復旧完了: {} 個のファイルを保存しました", counter);
    println!("実行時間: {:.2?}", duration);
}

fn match_start(buffer: &[u8]) -> Option<FileType> {
    if buffer.len() >= JPEG_START.len() && buffer.starts_with(JPEG_START) {
        Some(FileType::Jpeg)
    } else if buffer.len() >= RW2_START.len() && buffer.starts_with(RW2_START) {
        Some(FileType::Rw2)
    } else {
        None
    }
}

fn find_next_start(buffer: &[u8]) -> Option<usize> {
    for i in 0..buffer.len() {
        if buffer[i..].starts_with(JPEG_START) || buffer[i..].starts_with(RW2_START) {
            return Some(i);
        }
    }
    None
}

fn save_file(data: &[u8], counter: usize, file_type: FileType) {
    let ext = match file_type {
        FileType::Jpeg => "jpg",
        FileType::Rw2 => "rw2",
    };

    let filename = format!("{}/image_{:06}.{}", OUTPUT_DIR, counter, ext);
    match File::create(&filename) {
        Ok(mut out_file) => {
            if let Err(e) = out_file.write_all(data) {
                eprintln!("ファイル書き込みエラー: {}", e);
            } else {
                println!("Saved: {}", filename);
            }
        }
        Err(e) => eprintln!("ファイル作成エラー: {}", e),
    }
}
