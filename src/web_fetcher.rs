//! Web page source fetching module
//!
//! Provides fetching web page source code, detecting encodings, and parsing HTML structure.
//! Exposed to the frontend JS via the bridge API.

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use std::io::Read;

/// Fetches the web page source code
///
/// # Arguments
/// - `url`: The complete web page URL
///
/// # Returns
/// JSON containing the source content, status code, response headers, etc.
pub fn fetch_page(url: &str) -> Result<JsonValue> {
    // Validate the URL
    let url = url.trim();
    if url.is_empty() {
        return Err(anyhow::anyhow!("URL cannot be empty"));
    }

    // Auto-complete the protocol
    let full_url = if !url.starts_with("http://") && !url.starts_with("https://") {
        format!("https://{}", url)
    } else {
        url.to_string()
    };

    log::info!("Fetching web page: {}", full_url);

    // Send the HTTP request using ureq
    let resp = ureq::get(&full_url)
        .timeout(std::time::Duration::from_secs(30))
        .call()
        .with_context(|| format!("HTTP request failed: {}", full_url))?;

    let status = resp.status();
    let status_text = resp.status_text().to_string();

    // Collect response headers
    let mut headers = serde_json::Map::new();
    for header_name in &["content-type", "content-length", "server", "date", "last-modified", "etag", "set-cookie"] {
        if let Some(val) = resp.header(header_name) {
            headers.insert(header_name.to_string(), JsonValue::String(val.to_string()));
        }
    }

    // Read the response body
    let mut body = Vec::new();
    resp.into_reader()
        .take(10 * 1024 * 1024) // Maximum 10MB
        .read_to_end(&mut body)
        .context("Failed to read response body")?;

    // Detect the encoding and convert to UTF-8
    let (html, encoding) = detect_and_decode(&body);

    // Extract basic information
    let title = extract_title(&html);
    let meta_description = extract_meta(&html, "description");
    let meta_keywords = extract_meta(&html, "keywords");
    let link_count = count_tag(&html, "a");
    let image_count = count_tag(&html, "img");
    let script_count = count_tag(&html, "script");
    let style_count = count_tag(&html, "link") + count_tag(&html, "style");

    let result = serde_json::json!({
        "success": true,
        "url": full_url,
        "status": status,
        "status_text": status_text,
        "headers": headers,
        "encoding": encoding,
        "html": html,
        "size_bytes": body.len(),
        "title": title,
        "meta": {
            "description": meta_description,
            "keywords": meta_keywords
        },
        "stats": {
            "links": link_count,
            "images": image_count,
            "scripts": script_count,
            "styles": style_count
        }
    });

    Ok(result)
}

/// Detects the encoding and decodes into a UTF-8 string
fn detect_and_decode(data: &[u8]) -> (String, String) {
    // Try to extract the encoding declaration from the HTML
    let html_start = String::from_utf8_lossy(&data[..data.len().min(4096)]);
    let declared_encoding = extract_encoding(&html_start);
    
    // Try decoding with the detected encoding
    if let Some(enc) = &declared_encoding {
        if let Some((decoded, _, _)) = decode_with_encoding(data, enc) {
            if !decoded.is_empty() {
                return (decoded, enc.clone());
            }
        }
    }

    // Try UTF-8
    if let Ok(s) = String::from_utf8(data.to_vec()) {
        return (s, "UTF-8".to_string());
    }

    // Try GBK/GB2312 (common on Chinese websites)
    if let Some((decoded, _, _)) = decode_with_encoding(data, "gbk") {
        if !decoded.is_empty() && decoded.chars().any(|c| c > '\u{4e00}' && c < '\u{9fff}') {
            return (decoded, "GBK".to_string());
        }
    }

    // Try Latin-1
    if let Some((decoded, _, _)) = decode_with_encoding(data, "windows-1252") {
        return (decoded, "Windows-1252".to_string());
    }

    // Fallback: use a lossy UTF-8 conversion
    let fallback = String::from_utf8_lossy(data).to_string();
    (fallback, declared_encoding.unwrap_or_else(|| "UTF-8 (lossy)".to_string()))
}

/// Decodes byte data using the specified encoding
fn decode_with_encoding(data: &[u8], encoding: &str) -> Option<(String, usize, bool)> {
    let encoder = encoding_rs::Encoding::for_label(encoding.as_bytes())?;
    let (result, _, had_errors) = encoder.decode(data);
    Some((result.into_owned(), data.len(), had_errors))
}

/// Extracts the encoding declaration from the HTML
fn extract_encoding(html: &str) -> Option<String> {
    // Check <meta charset="...">
    if let Some(start) = html.find("charset") {
        let after = &html[start..];
        if let Some(quote_start) = after.find(['"', '\'']) {
            let after_quote = &after[quote_start + 1..];
            if let Some(quote_end) = after_quote.find(['"', '\'', '>', ' ']) {
                let enc = after_quote[..quote_end].to_string();
                if !enc.is_empty() && enc.len() < 20 {
                    return Some(enc);
                }
            }
        }
    }
    // Check <meta http-equiv="Content-Type" content="text/html; charset=...">
    if let Some(start) = html.to_lowercase().find("content-type") {
        let after = &html[start..];
        if let Some(charset_pos) = after.to_lowercase().find("charset=") {
            let after_charset = &after[charset_pos + 8..];
            let enc: String = after_charset.chars()
                .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            if !enc.is_empty() {
                return Some(enc);
            }
        }
    }
    None
}

/// Extracts the web page title
fn extract_title(html: &str) -> String {
    if let Some(start) = html.find("<title") {
        // Find the closing >
        if let Some(tag_end) = html[start..].find('>') {
            let content_start = start + tag_end + 1;
            if let Some(end) = html[content_start..].find("</title") {
                let title = html[content_start..content_start + end].trim();
                return title.to_string();
            }
        }
    }
    String::new()
}

/// Extracts the content of a meta tag
fn extract_meta(html: &str, name: &str) -> String {
    let lower = html.to_lowercase();
    let patterns = [
        format!("name=\"{}\"", name),
        format!("name='{}'", name),
        format!("property=\"og:{}\"", name),
        format!("property='og:{}'", name),
    ];

    for pattern in &patterns {
        if let Some(pos) = lower.find(pattern.as_str()) {
            // Look for the content attribute nearby
            let search_start = pos.saturating_sub(200);
            let search_end = (pos + pattern.len() + 200).min(html.len());
            let area = &html[search_start..search_end];
            
            if let Some(content_start) = area.find("content=\"") {
                let after = &area[content_start + 9..];
                if let Some(end) = after.find('"') {
                    return after[..end].to_string();
                }
            }
            if let Some(content_start) = area.find("content='") {
                let after = &area[content_start + 9..];
                if let Some(end) = after.find('\'') {
                    return after[..end].to_string();
                }
            }
        }
    }
    String::new()
}

/// Counts the number of a given tag in the HTML
fn count_tag(html: &str, tag: &str) -> usize {
    let open_tag = format!("<{} ", tag);
    let self_close = format!("<{}/>", tag);
    let close_tag = format!("</{}>", tag);
    
    let count_open = html.matches(&open_tag).count();
    let count_self = html.matches(&self_close).count();
    let count_close = html.matches(&close_tag).count();
    
    // For self-closing tags, use their own count; for non-self-closing tags, use the non-self-closing open tag count
    if tag == "img" || tag == "link" {
        count_open + count_self
    } else {
        count_open.max(count_close)
    }
}
