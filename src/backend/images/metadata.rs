fn parse_image_info(path: &Path, bytes: &[u8]) -> (Option<ImageDimensions>, Vec<PngTextChunk>) {
    match path.extension().and_then(OsStr::to_str) {
        Some(ext) if ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg") => {
            (parse_jpeg_dimensions(bytes), Vec::new())
        }
        _ => parse_png_info(bytes),
    }
}

fn parse_jpeg_dimensions(bytes: &[u8]) -> Option<ImageDimensions> {
    if bytes.len() < 4 || bytes[0] != 0xff || bytes[1] != 0xd8 {
        return None;
    }
    let mut pos = 2usize;
    while pos + 3 < bytes.len() {
        while pos < bytes.len() && bytes[pos] == 0xff {
            pos += 1;
        }
        let marker = *bytes.get(pos)?;
        pos += 1;
        if marker == 0xd9 || marker == 0xda {
            break;
        }
        let len = u16::from_be_bytes([*bytes.get(pos)?, *bytes.get(pos + 1)?]) as usize;
        if len < 2 || pos.checked_add(len)? > bytes.len() {
            break;
        }
        let data = &bytes[pos + 2..pos + len];
        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) && data.len() >= 5
        {
            let height = u16::from_be_bytes([data[1], data[2]]) as u32;
            let width = u16::from_be_bytes([data[3], data[4]]) as u32;
            return Some(ImageDimensions { width, height });
        }
        pos += len;
    }
    None
}

fn parse_png_info(bytes: &[u8]) -> (Option<ImageDimensions>, Vec<PngTextChunk>) {
    const PNG_SIG: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 8 || &bytes[..8] != PNG_SIG {
        return (None, Vec::new());
    }
    let mut pos = 8usize;
    let mut dimensions = None;
    let mut text = Vec::new();
    let mut seen_text = HashSet::new();
    while pos.checked_add(12).is_some_and(|end| end <= bytes.len()) {
        let len = u32::from_be_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        let chunk_type = &bytes[pos + 4..pos + 8];
        let data_start = pos + 8;
        let Some(data_end) = data_start.checked_add(len) else {
            break;
        };
        let Some(next_pos) = data_end.checked_add(4) else {
            break;
        };
        if next_pos > bytes.len() {
            break;
        }
        let data = &bytes[data_start..data_end];
        match chunk_type {
            b"IHDR" if data.len() >= 8 => {
                dimensions = Some(ImageDimensions {
                    width: u32::from_be_bytes(data[0..4].try_into().unwrap()),
                    height: u32::from_be_bytes(data[4..8].try_into().unwrap()),
                });
            }
            b"tEXt" => {
                if let Some((keyword, value)) = split_png_text_pair(data) {
                    push_unique_png_text(&mut text, &mut seen_text, keyword, value);
                }
            }
            b"zTXt" => {
                if let Some((keyword, value)) = parse_ztxt_chunk(data) {
                    push_unique_png_text(&mut text, &mut seen_text, keyword, value);
                }
            }
            b"iTXt" => {
                if let Some((keyword, value)) = parse_itxt_chunk(data) {
                    push_unique_png_text(&mut text, &mut seen_text, keyword, value);
                }
            }
            b"IEND" => break,
            _ => {}
        }
        pos = next_pos;
    }
    (dimensions, text)
}

fn push_unique_png_text(
    text: &mut Vec<PngTextChunk>,
    seen: &mut HashSet<(String, String)>,
    keyword: String,
    value: String,
) {
    if seen.insert((keyword.clone(), value.clone())) {
        text.push(PngTextChunk { keyword, value });
    }
}

fn split_png_text_pair(data: &[u8]) -> Option<(String, String)> {
    let nul = data.iter().position(|b| *b == 0)?;
    let keyword = png_text_lossy(&data[..nul]);
    let value = png_text_lossy(&data[nul + 1..]);
    (!keyword.is_empty()).then_some((keyword, value))
}

fn parse_ztxt_chunk(data: &[u8]) -> Option<(String, String)> {
    let nul = data.iter().position(|b| *b == 0)?;
    let keyword = png_text_lossy(&data[..nul]);
    let compressed = data.get(nul + 2..)?;
    let value = inflate_png_text(compressed)
        .unwrap_or_else(|| "[compressed text decode failed]".to_string());
    (!keyword.is_empty()).then_some((keyword, value))
}

fn parse_itxt_chunk(data: &[u8]) -> Option<(String, String)> {
    let keyword_end = data.iter().position(|b| *b == 0)?;
    let keyword = png_text_lossy(&data[..keyword_end]);
    let mut pos = keyword_end + 1;
    let compression_flag = *data.get(pos)?;
    pos += 2; // compression flag + compression method
    let lang_end = data.get(pos..)?.iter().position(|b| *b == 0)? + pos;
    pos = lang_end + 1;
    let translated_end = data.get(pos..)?.iter().position(|b| *b == 0)? + pos;
    pos = translated_end + 1;
    let raw_text = data.get(pos..)?;
    let value = if compression_flag == 1 {
        inflate_png_text(raw_text).unwrap_or_else(|| "[compressed iTXt decode failed]".to_string())
    } else {
        String::from_utf8_lossy(raw_text).to_string()
    };
    (!keyword.is_empty()).then_some((keyword, value))
}

fn inflate_png_text(data: &[u8]) -> Option<String> {
    let mut decoder = flate2::read::ZlibDecoder::new(data);
    let mut out = String::new();
    decoder.read_to_string(&mut out).ok()?;
    Some(out)
}

fn png_text_lossy(data: &[u8]) -> String {
    String::from_utf8_lossy(data).trim().to_string()
}

async fn compute_image_stats(dir: &Path) -> Result<ImageStats, StatusCode> {
    let mut total_images = 0usize;
    let mut png_stems = HashSet::new();
    let mut candidate_files: Vec<(String, u64, bool)> = Vec::new();
    let mut read_dir = tokio::fs::read_dir(dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        let path = entry.path();
        let Some(ext) = path.extension().and_then(OsStr::to_str) else {
            continue;
        };
        if !matches_image_ext(ext) {
            continue;
        }
        let Ok(metadata) = tokio::fs::symlink_metadata(&path).await else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(OsStr::to_str).map(str::to_owned) else {
            continue;
        };
        let len = metadata.len();
        if is_source_image_ext(ext) {
            total_images += 1;
            png_stems.insert(stem.clone());
            candidate_files.push((stem, len, true));
        } else {
            candidate_files.push((related_base_stem(&stem).to_string(), len, false));
        }
    }

    let total_bytes = candidate_files
        .into_iter()
        .filter(|(stem, _, is_png)| *is_png || png_stems.contains(stem))
        .fold(0u64, |sum, (_, len, _)| sum.saturating_add(len));

    Ok(ImageStats {
        total_images,
        total_bytes,
    })
}

fn matches_image_ext(ext: &str) -> bool {
    is_source_image_ext(ext) || ext.eq_ignore_ascii_case("heic") || ext.eq_ignore_ascii_case("webp")
}

fn related_base_stem(stem: &str) -> &str {
    stem.strip_suffix("_preview")
        .or_else(|| {
            let (base, quality) = stem.rsplit_once("_q")?;
            quality
                .chars()
                .all(|ch| ch.is_ascii_digit())
                .then_some(base)
        })
        .unwrap_or(stem)
}
