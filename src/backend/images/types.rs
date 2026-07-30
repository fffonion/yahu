#[derive(Debug, Clone, Serialize)]
struct ImageEntry {
    filename: String,
    heic_filename: Option<String>,
    image_url: String,
    png_url: String,
    heic_url: Option<String>,
    heic_status: String,
    download_filename: String,
    download_url: String,
    download_label: String,
    created_at: i64,
    modified_at: i64,
    size: u64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct ListQuery {
    offset: Option<usize>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
struct RefreshQuery {
    after: Option<i64>,
    limit: Option<usize>,
    check: Option<String>,
}

#[derive(Debug, Serialize)]
struct RefreshResult {
    new_items: Vec<ImageEntry>,
    checked_items: Vec<ImageEntry>,
}

#[derive(Debug, Deserialize)]
struct BatchRequest {
    filenames: Vec<String>,
}

#[derive(Debug, Serialize)]
struct BatchResult {
    success_count: usize,
    fail_count: usize,
    errors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    download_pngs: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct ImageStats {
    total_images: usize,
    total_bytes: u64,
}

#[derive(Debug, Serialize)]
struct ImageDimensions {
    width: u32,
    height: u32,
}

#[derive(Debug, Serialize)]
struct FileMetadata {
    filename: String,
    url: String,
    size: u64,
    modified_at: i64,
}

#[derive(Debug, Serialize)]
struct PngTextChunk {
    keyword: String,
    value: String,
}

#[derive(Debug, Serialize)]
struct ImageMetadata {
    filename: String,
    dimensions: Option<ImageDimensions>,
    png: FileMetadata,
    webp: Option<FileMetadata>,
    heic: Option<FileMetadata>,
    heic_status: String,
    png_text: Vec<PngTextChunk>,
}
