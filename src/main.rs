use std::fs::{File, create_dir_all};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Instant;

const DEVICE_PATH: &str = "/dev/mmcblk0";
const OUTPUT_DIR: &str = "recovered";
const JPEG_START: &[u8] = &[0xFF, 0xD8];
const JPEG_END: &[u8] = &[0xFF, 0xD9];
const RW2_START: &[u8] = &[0x49, 0x49, 0x2A, 0x00];
const READ_BLOCK_SIZE: usize = 512 * 1024;

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

    loop {
        match file.read(&mut temp) {
            Ok(0) => break,
            Ok(n) => buffer.extend_from_slice(&temp[..n]),
            Err(e) => {
                eprintln!("読み取りエラー: {}", e);
                break;
            }
        }

        loop {
            let candidates = find_all_starts(&buffer);

            if candidates.is_empty() {
                // スタートシグネチャ見つからなければ、末尾だけ残して次ブロックへ
                buffer = buffer.split_off(buffer.len().saturating_sub(RW2_START.len()));
                break;
            }

            let (start_idx, file_type) = &candidates[0];

            // JPEGの場合、エンドマーカーを探して保存
            if *file_type == FileType::Jpeg {
                match find_signature(&buffer[*start_idx + JPEG_START.len()..], JPEG_END) {
                    Some(offset) => {
                        let end_idx = *start_idx + JPEG_START.len() + offset + JPEG_END.len();
                        save_file(&buffer[*start_idx..end_idx], counter, file_type);
                        counter += 1;
                        buffer = buffer.split_off(end_idx);
                    }
                    None => {
                        buffer = buffer.split_off(*start_idx);
                        break;
                    }
                }
            }
            // RW2の場合、次のスタートシグネチャまでを保存
            else {
                let next_candidates = find_all_starts(&buffer[*start_idx + 4..]);
                let end_idx = match next_candidates.first() {
                    Some((next_idx, _)) => *start_idx + 4 + *next_idx,
                    None => buffer.len(),
                };
                save_file(&buffer[*start_idx..end_idx], counter, file_type);
                counter += 1;
                buffer = buffer.split_off(end_idx);
            }
        }
    }

    let duration = start_time.elapsed();
    println!("\n復旧完了: {} 個のファイルを保存しました", counter);
    println!("実行時間: {:.2?}", duration);
}

fn find_signature(buffer: &[u8], signature: &[u8]) -> Option<usize> {
    buffer.windows(signature.len()).position(|window| window == signature)
}

fn find_all_starts(buffer: &[u8]) -> Vec<(usize, FileType)> {
    let mut results = Vec::new();

    if let Some(idx) = find_signature(buffer, JPEG_START) {
        results.push((idx, FileType::Jpeg));
    }
    if let Some(idx) = find_signature(buffer, RW2_START) {
        results.push((idx, FileType::Rw2));
    }

    results.sort_by_key(|k| k.0);
    results
}

fn save_file(data: &[u8], counter: usize, file_type: &FileType) {
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

#[cfg(test)]
mod tests {

    use super::*;

    const JPEG_START: &[u8] = &[0xFF, 0xD8];

    // ---------------------------
    // Tests for find_signature
    // ---------------------------

    #[test]
    fn test_should_return_correct_index_when_signature_is_found() {
        // 1. setup
        let buffer = [0x00, 0x11, 0x22, 0xFF, 0xD8, 0x33, 0x44];
        let signature = JPEG_START;

        // 2. execute
        let result = find_signature(&buffer, signature);
        println!("{:?}", result);

        // 3. verify
        assert_eq!(result, Some(3));
    }

    #[test]
    fn test_should_return_none_when_signature_is_not_found() {
        // 1. setup
        let buffer = [0x00, 0x11, 0x22, 0x33, 0x44];
        let signature = JPEG_START;

        // 2. execute
        let result = find_signature(&buffer, signature);

        // 3. verify
        assert_eq!(result, None);
    }

    #[test]
    fn test_should_return_first_occurrence_when_multiple_matches_exist() {
        // 1. setup
        let buffer = [0xFF, 0xD8, 0x00, 0xFF, 0xD8, 0x01];
        let signature = JPEG_START;

        // 2. execute
        let result = find_signature(&buffer, signature);

        // 3. verify
        assert_eq!(result, Some(0)); // 最初の0番目のマッチを返す
    }

    // ---------------------------
    // Tests for find_all_starts
    // ---------------------------

    #[test]
    fn test_should_return_jpeg_and_rw2_when_both_signatures_exist() {
        // 1. setup
        let buffer = [
            0x00, 0xFF, 0xD8, 0x01, 0x49, 0x49, 0x2A, 0x00, 0x02,
        ];

        // 2. execute
        let results = find_all_starts(&buffer);

        // 3. verify
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], (1, FileType::Jpeg));
        assert_eq!(results[1], (4, FileType::Rw2));
    }

    #[test]
    fn test_should_return_only_jpeg_when_only_jpeg_signature_exists() {
        // 1. setup
        let buffer = [0xFF, 0xD8, 0x10, 0x20];

        // 2. execute
        let results = find_all_starts(&buffer);

        // 3. verify
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], (0, FileType::Jpeg));
    }

    #[test]
    fn test_should_return_only_rw2_when_only_rw2_signature_exists() {
        // 1. setup
        let buffer = [0x49, 0x49, 0x2A, 0x00, 0xAA, 0xBB];

        // 2. execute
        let results = find_all_starts(&buffer);

        // 3. verify
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], (0, FileType::Rw2));
    }

    #[test]
    fn test_should_return_empty_when_no_signatures_exist() {
        // 1. setup
        let buffer = [0x00, 0x01, 0x02, 0x03, 0x04];

        // 2. execute
        let results = find_all_starts(&buffer);

        // 3. verify
        assert_eq!(results.len(), 0);
    }
}