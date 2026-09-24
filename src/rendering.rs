//! Rendering optimization module
//!
//! Provides WebView rendering performance optimization configuration and utility functions.
//! Helps handle rendering of a large number of entities and prevents lag.

use serde::{Deserialize, Serialize};

/// Rendering optimization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderConfig {
    /// Enable hardware acceleration (enabled by default)
    pub hardware_acceleration: bool,
    
    /// Rendering frame rate limit (0 = no limit)
    pub frame_rate_limit: u32,
    
    /// Enable smooth scrolling
    pub smooth_scrolling: bool,
    
    /// Rendering mode
    pub rendering_mode: RenderingMode,
    
    /// Virtual scrolling threshold for automatically handling large lists
    /// When the number of list items exceeds this value, virtual scrolling is enabled automatically
    pub virtual_scroll_threshold: usize,
    
    /// Enable WebGL 2.0 support
    pub webgl2_enabled: bool,
    
    /// Enable CSS hardware acceleration
    pub css_animation_optimized: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            hardware_acceleration: true,
            frame_rate_limit: 0, // 0 = no limit, use the system default
            smooth_scrolling: true,
            rendering_mode: RenderingMode::Auto,
            virtual_scroll_threshold: 100,
            webgl2_enabled: true,
            css_animation_optimized: true,
        }
    }
}

/// Rendering mode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RenderingMode {
    /// Automatically select based on system performance
    Auto,
    /// High performance mode (higher resource usage)
    HighPerformance,
    /// Power saving mode (lower frame rate, limited animations)
    PowerSaving,
}

impl RenderingMode {
    /// Get the recommended frame rate limit
    pub fn recommended_frame_limit(&self) -> u32 {
        match self {
            RenderingMode::Auto => 0, // no limit
            RenderingMode::HighPerformance => 60,
            RenderingMode::PowerSaving => 30,
        }
    }
    
    /// Get whether animations are enabled
    pub fn animations_enabled(&self) -> bool {
        match self {
            RenderingMode::PowerSaving => false,
            _ => true,
        }
    }
}

/// Generate JavaScript code for rendering optimization
///
/// Injected into the page to dynamically optimize frontend rendering performance.
pub fn generate_render_optimization_script(config: &RenderConfig) -> String {
    let mut script = String::new();
    
    // Frame rate control
    if config.frame_rate_limit > 0 {
        script.push_str(&format!(
            "// Nefu rendering optimization: frame rate limit {}fps\n",
            config.frame_rate_limit
        ));
        script.push_str(&format!(
            "window.__nefu_fps_limit = {};\n",
            config.frame_rate_limit
        ));
    }
    
    // Rendering mode
    script.push_str(&format!(
        "window.__nefu_render_mode = '{}';\n",
        match config.rendering_mode {
            RenderingMode::Auto => "auto",
            RenderingMode::HighPerformance => "high-performance",
            RenderingMode::PowerSaving => "power-saving",
        }
    ));
    
    // Smooth scrolling
    if config.smooth_scrolling {
        script.push_str("// Nefu rendering optimization: enable smooth scrolling\n");
        script.push_str("document.documentElement.style.scrollBehavior = 'smooth';\n");
    }
    
    // CSS animation optimization
    if config.css_animation_optimized {
        script.push_str("// Nefu rendering optimization: CSS animation optimization\n");
        script.push_str(
            "const style = document.createElement('style');\
             style.textContent = `\
             .nefu-optimized * {\
                 -webkit-transform: translateZ(0);\
                 transform: translateZ(0);\
                 will-change: transform;\
             }\
             .nefu-gpu-accelerated {\
                 -webkit-transform: translateZ(0);\
                 transform: translateZ(0);\
                 will-change: transform, opacity;\
                 backface-visibility: hidden;\
             }\
             @media (prefers-reduced-motion: reduce) {\
                 * {\
                     animation-duration: 0.01ms !important;\
                     transition-duration: 0.01ms !important;\
                 }\
             }\
             `;\
             document.head.appendChild(style);\
             document.documentElement.classList.add('nefu-optimized');\n"
        );
    }
    
    // Large entity detection and virtual scrolling hints
    script.push_str(&format!(
        "// Nefu rendering optimization: virtual scroll threshold {}\n",
        config.virtual_scroll_threshold
    ));
    script.push_str(&format!(
        "window.__nefu_virtual_scroll_threshold = {};\n",
        config.virtual_scroll_threshold
    ));
    script.push_str(
        "// Automatically detect large lists and suggest using virtual scrolling\n\
         const observer = new MutationObserver((mutations) => {\
             const lists = document.querySelectorAll('ul, ol, table, [role=\"list\"]');\
             lists.forEach(list => {\
                 const itemCount = list.children.length;\
                 if (itemCount > window.__nefu_virtual_scroll_threshold) {\
                     console.warn(`[Nefu] Detected ${itemCount} list items, consider using virtual scrolling to optimize performance`);\
                 }\
             });\
         });\n"
    );
    script.push_str(
        "if (document.body) {\
             observer.observe(document.body, { childList: true, subtree: true });\
         } else {\
             document.addEventListener('DOMContentLoaded', () => {\
                 observer.observe(document.body, { childList: true, subtree: true });\
             });\
         }\n"
    );
    
    // Visibility-based lazy loading
    script.push_str(
        "// Nefu rendering optimization: visibility-based lazy loading\n\
         const lazyImages = document.querySelectorAll('img[data-src], [data-lazy]');\
         if ('IntersectionObserver' in window) {\
             const imgObserver = new IntersectionObserver((entries) => {\
                 entries.forEach(entry => {\
                     if (entry.isIntersecting) {\
                         const img = entry.target;\
                         if (img.dataset.src) {\
                             img.src = img.dataset.src;\
                         }\
                         img.removeAttribute('data-lazy');\
                         imgObserver.unobserve(img);\
                     }\
                 });\
             });\
             lazyImages.forEach(img => imgObserver.observe(img));\
         }\n"
    );
    
    script
}

/// Apply the render configuration to the wry WebView builder
///
/// Sets various WebView-level rendering options.
pub fn apply_render_config<'a>(builder: wry::WebViewBuilder<'a>, _config: &RenderConfig) -> wry::WebViewBuilder<'a> {
    let mut builder = builder;
    
    // Note: wry's specific API may vary by version
    // A generic optimization framework is provided here
    
    // The current wry version may not have a direct hardware acceleration API
    // Hardware acceleration is usually managed automatically by the OS and the WebView engine
    
    builder
}

/// Create CSS optimized for a large number of entities
///
/// Provides performance optimization styles for pages containing a large number of DOM elements.
pub fn generate_performance_css() -> String {
    r#"/* Nefu performance optimization CSS */

/* Enable GPU-accelerated compositing layers */
.nefu-gpu-layer {
    transform: translateZ(0);
    will-change: transform;
    backface-visibility: hidden;
}

/* Optimize scrolling performance */
.nefu-smooth-scroll {
    scroll-behavior: smooth;
    -webkit-overflow-scrolling: touch;
    overscroll-behavior: contain;
}

/* Reduce repaints and reflows */
.nefu-contain-strict {
    contain: strict;
}

.nefu-contain-layout {
    contain: layout;
}

.nefu-contain-content {
    contain: content;
}

/* Large list optimization */
.nefu-large-list {
    contain: layout style paint;
    content-visibility: auto;
    contain-intrinsic-size: 0 500px;
}

/* Virtualization support */
.nefu-virtual-scroll-container {
    position: relative;
    overflow: auto;
    contain: strict;
}

.nefu-virtual-scroll-item {
    position: absolute;
    left: 0;
    right: 0;
}

/* Limit animation effects to save performance */
@media (prefers-reduced-motion: reduce) {
    *,
    *::before,
    *::after {
        animation-duration: 0.01ms !important;
        animation-iteration-count: 1 !important;
        transition-duration: 0.01ms !important;
        scroll-behavior: auto !important;
    }
}

/* Performance mode class */
.nefu-power-saving * {
    animation: none !important;
    transition: none !important;
    box-shadow: none !important;
    filter: none !important;
}

.nefu-high-performance {
    /* In high performance mode, animations can be used more aggressively */
}
"#.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_render_config_default() {
        let config = RenderConfig::default();
        assert!(config.hardware_acceleration);
        assert_eq!(config.frame_rate_limit, 0);
        assert!(config.smooth_scrolling);
    }
    
    #[test]
    fn test_rendering_mode() {
        assert_eq!(RenderingMode::Auto.recommended_frame_limit(), 0);
        assert_eq!(RenderingMode::HighPerformance.recommended_frame_limit(), 60);
        assert_eq!(RenderingMode::PowerSaving.recommended_frame_limit(), 30);
    }
    
    #[test]
    fn test_generate_script() {
        let config = RenderConfig::default();
        let script = generate_render_optimization_script(&config);
        assert!(!script.is_empty());
        assert!(script.contains("__nefu_render_mode"));
    }
    
    #[test]
    fn test_generate_css() {
        let css = generate_performance_css();
        assert!(!css.is_empty());
        assert!(css.contains("GPU"));
    }
}
