//! Nefu Coding (.nc) language parser
//!
//! .nc is a declarative UI scripting language supporting two syntaxes:
//!
//! ## New syntax (v2)
//! ```text
//! import<nefu.nch>        \ import the main module
//! \ This is a single-line comment
//! /* This is a multi-line comment */
//! main"                    \ main function
//!   nefu("list",[1,2,3,4],BLACK)  \ call a JS function
//!   nefu("alert","hello")
//! end(all);                \ end
//! ```
//!
//! ## Old syntax (JSON compatible)
//! ```json
//! { "component": "button", "text": "Submit", "color": "primary" }
//! ```

use anyhow::{bail, Context, Result};

/// Bootstrap 5.3.0 CDN URL
const BOOTSTRAP_CSS_CDN: &str = "https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/css/bootstrap.min.css";
const BOOTSTRAP_JS_CDN: &str = "https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/js/bootstrap.bundle.min.js";

/// Font Awesome 6.5.0 CDN URL
const FONTAWESOME_CDN: &str = "https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.5.0/css/all.min.css";

/// Google Fonts Inter
const GOOGLE_FONTS_URL: &str = "https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&display=swap";

/// Parse .nc content into a complete HTML page
///
/// Automatically detects the syntax type (v2 new syntax or JSON old syntax),
/// and uses the corresponding parser to generate HTML.
///
/// # Parameters
/// - `nc_content`: the content of the .nc file
///
/// # Returns
/// The complete HTML page string
pub fn parse_nc_to_html(nc_content: &str) -> Result<String> {
    let trimmed = nc_content.trim();

    // Detect the syntax type: new syntax starts with import<, main", or \
    if is_new_syntax(trimmed) {
        parse_new_syntax(trimmed)
    } else {
        // Old syntax: JSON format
        parse_old_syntax(trimmed)
    }
}

/// Detect whether the content uses the new syntax
///
/// Characteristics of the new syntax:
/// - Starts with `import<`
/// - Starts with `main"`
/// - Starts with `\` (comment line)
fn is_new_syntax(content: &str) -> bool {
    let trimmed = content.trim_start();
    trimmed.starts_with("import<")
        || trimmed.starts_with("main\"")
        || trimmed.starts_with("main'")
        || trimmed.starts_with('\\')
        || trimmed.starts_with("/*")
}

/// Parse the new syntax and generate HTML
///
/// The new syntax is a script-like language compiled to JavaScript code,
/// which is then embedded into an HTML page.
fn parse_new_syntax(content: &str) -> Result<String> {
    // Step 1: Remove comments and parse the structure
    let cleaned = remove_comments(content);
    let lines: Vec<&str> = cleaned.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    // Parse import statements
    let mut imports = Vec::new();
    let mut in_main = false;
    // Collect all JS code
    let mut js_lines: Vec<String> = Vec::new();

    for line in &lines {
        if line.starts_with("import<") {
            // Parse an import statement
            if let Some(inner) = line.strip_prefix("import<") {
                let module = inner.trim_end_matches('>').trim();
                imports.push(module.to_string());
            }
        } else if line.starts_with("main\"") || line.starts_with("main'") {
            // Main function starts
            in_main = true;
            js_lines.push("// main function".to_string());
            js_lines.push("(async () => {".to_string());
        } else if line.starts_with("end(") {
            // End statement
            if in_main {
                js_lines.push("})();".to_string());
                in_main = false;
            }
        } else if in_main && line.starts_with("nefu(") {
            // Parse a nefu function call
            let js_call = parse_nefu_call(line)?;
            js_lines.push(format!("  try {{ {} }} catch(e) {{ console.error('nefu call failed:', e); }}", js_call));
        } else if in_main && !line.is_empty() {
            // Other code lines - used directly as JS code
            js_lines.push(format!("  {}", line));
        }
    }

    // If main did not end properly, close it automatically
    if in_main {
        js_lines.push("})();".to_string());
    }

    // Extract the page title
    let title = imports.first().map(|s| s.as_str()).unwrap_or("Nefu App");

    // Generate HTML
    let js_code = js_lines.join("\n");
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title}</title>
    <link rel="stylesheet" href="{bootstrap_css}">
    <link rel="stylesheet" href="{fontawesome}">
    <link href="{google_fonts}" rel="stylesheet">
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: 'Inter', -apple-system, sans-serif; background: #f5f5f5; color: #333; }}
        #app {{ min-height: 100vh; display: flex; flex-direction: column; }}
    </style>
</head>
<body>
    <div id="app">
        <div id="nefu-content"></div>
    </div>
    <script src="{bootstrap_js}"></script>
    <script>
{js_code}
    </script>
</body>
</html>"#,
        title = title,
        bootstrap_css = BOOTSTRAP_CSS_CDN,
        fontawesome = FONTAWESOME_CDN,
        google_fonts = GOOGLE_FONTS_URL,
        bootstrap_js = BOOTSTRAP_JS_CDN,
        js_code = js_code,
    );

    Ok(html)
}

/// Remove comments (single-line \ and multi-line /* */)
fn remove_comments(content: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = content.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Single-line comment: \
        if chars[i] == '\\' && i + 1 < len && chars[i + 1] != '\\' {
            // Skip to the end of the line
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Multi-line comment: /* */
        if chars[i] == '/' && i + 1 < len && chars[i + 1] == '*' {
            i += 2; // Skip /*
            while i + 1 < len {
                if chars[i] == '*' && chars[i + 1] == '/' {
                    i += 2; // Skip */
                    break;
                }
                i += 1;
            }
            continue;
        }

        result.push(chars[i]);
        i += 1;
    }

    result
}

/// Parse a nefu() function call into JavaScript code
///
/// Input: nefu("list",[1,2,3,4],BLACK)
/// Output: await nefu.invoke("list", [1,2,3,4], "BLACK")
fn parse_nefu_call(line: &str) -> Result<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with("nefu(") {
        bail!("Invalid nefu call: {}", line);
    }

    // Extract the content inside the parentheses
    let inner = trimmed
        .strip_prefix("nefu(")
        .and_then(|s| {
            // Find the matching closing parenthesis
            let mut depth = 1;
            for (i, c) in s.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(&s[..i]);
                        }
                    }
                    _ => {}
                }
            }
            None
        })
        .ok_or_else(|| anyhow::anyhow!("nefu() unbalanced parentheses: {}", line))?;

    if inner.is_empty() {
        bail!("nefu() missing arguments: {}", line);
    }

    // Parse the arguments
    let args = parse_nefu_args(inner);

    // Build the JS call
    // The first argument is the method name, the rest are arguments
    let js_args: Vec<String> = args.iter().map(|a| a.to_js_string()).collect();
    let js = format!("await nefu.invoke({})", js_args.join(", "));

    Ok(js)
}

/// Parse the argument list of nefu()
///
/// Supports:
/// - Strings: "hello"
/// - Arrays: [1,2,3]
/// - Identifiers (constants): BLACK, RED, ALL
/// - Numbers: 123, 3.14
fn parse_nefu_args(input: &str) -> Vec<NefuArg> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut depth_paren = 0;
    let mut depth_bracket = 0;
    let mut in_string = false;
    let mut string_char = '"';
    let chars: Vec<char> = input.chars().collect();

    for &c in &chars {
        if in_string {
            current.push(c);
            if c == string_char {
                in_string = false;
            }
            continue;
        }

        match c {
            '"' | '\'' => {
                in_string = true;
                string_char = c;
                current.push(c);
            }
            '(' => {
                depth_paren += 1;
                current.push(c);
            }
            ')' => {
                depth_paren -= 1;
                current.push(c);
            }
            '[' => {
                depth_bracket += 1;
                current.push(c);
            }
            ']' => {
                depth_bracket -= 1;
                current.push(c);
            }
            ',' if depth_paren == 0 && depth_bracket == 0 => {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    args.push(parse_arg_value(&trimmed));
                }
                current.clear();
                continue;
            }
            _ => {
                current.push(c);
            }
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        args.push(parse_arg_value(&trimmed));
    }

    args
}

/// Parse a single argument value
fn parse_arg_value(value: &str) -> NefuArg {
    let v = value.trim();

    // String
    if (v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')) {
        let inner = &v[1..v.len()-1];
        return NefuArg::String(inner.to_string());
    }

    // Array
    if v.starts_with('[') && v.ends_with(']') {
        let inner = &v[1..v.len()-1];
        let items = parse_nefu_args(inner);
        return NefuArg::Array(items);
    }

    // Number
    if let Ok(n) = v.parse::<i64>() {
        return NefuArg::Number(n as f64);
    }
    if let Ok(f) = v.parse::<f64>() {
        return NefuArg::Number(f);
    }

    // Boolean
    if v == "true" || v == "false" {
        return NefuArg::Boolean(v == "true");
    }

    // Identifier (constant/variable)
    NefuArg::Identifier(v.to_string())
}

/// Nefu argument type
#[derive(Debug, Clone)]
enum NefuArg {
    String(String),
    Number(f64),
    Boolean(bool),
    Array(Vec<NefuArg>),
    Identifier(String),
}

impl NefuArg {
    /// Convert to a JS string representation
    fn to_js_string(&self) -> String {
        match self {
            NefuArg::String(s) => {
                // If the string starts with " or ', use it directly
                if s.starts_with('"') || s.starts_with('\'') {
                    s.clone()
                } else {
                    format!("\"{}\"", s)
                }
            }
            NefuArg::Number(n) => {
                if *n == n.floor() {
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                }
            }
            NefuArg::Boolean(b) => b.to_string(),
            NefuArg::Array(items) => {
                let items: Vec<String> = items.iter().map(|a| a.to_js_string()).collect();
                format!("[{}]", items.join(", "))
            }
            NefuArg::Identifier(id) => {
                // Uppercase identifiers are treated as constants and quoted; otherwise treated as variables
                if id.chars().all(|c| c.is_uppercase() || c == '_' || c.is_ascii_digit()) {
                    format!("\"{}\"", id)
                } else {
                    id.clone()
                }
            }
        }
    }
}

// ==================== Old syntax (JSON compatible) ====================

/// Old syntax: JSON format component parsing

use serde_json::Value;

/// List of all supported component types (including two named aliases)
const SUPPORTED_COMPONENTS: &[&str] = &[
    "button", "input", "textarea", "select", "checkbox", "radio", "radio-group",
    "table", "card", "alert", "modal", "navbar", "container",
    "row", "col", "text", "paragraph", "heading", "image", "img", "badge",
    "progress", "spinner", "tabs", "accordion", "carousel", "dropdown",
    "list", "list-group", "pagination", "breadcrumb", "tooltip", "popover",
    "toast", "offcanvas", "form", "nav", "footer", "header", "sidebar",
    "grid", "divider", "separator", "spacer", "icon", "code", "html", "raw",
    "page", "body",
];

/// Parse the old JSON syntax
fn parse_old_syntax(content: &str) -> Result<String> {
    // Parse JSON
    let root: Value = serde_json::from_str(content.trim())
        .context("NC file is not valid JSON format (when using new syntax, start with import< or main\")")?;

    // Verify that the root node is an object
    if !root.is_object() {
        bail!("NC root node must be a JSON object");
    }

    let component = Component::new(&root);

    // Normalize and validate the component type field (supports both component / type)
    let has_kind = root.get("component").and_then(|v| v.as_str()).is_some()
        || root.get("type").and_then(|v| v.as_str()).is_some();
    if !has_kind {
        bail!(
            "NC root node is missing the 'component' or 'type' field (use component: \"xxx\" or type: \"xxx\")"
        );
    }

    // Validate the component type
    if component.kind() != "raw" && !SUPPORTED_COMPONENTS.contains(&component.kind()) {
        log::warn!(
            "Unknown component type: '{}', will be rendered as a generic div",
            component.kind()
        );
    }

    // Recursively render the component tree
    let body_html = render_component(&component)?;

    // Assemble the complete HTML page (skip re-wrapping when the page root component already provides the HTML skeleton)
    if component.kind() == "page" && root.get("html").is_some() {
        return Ok(body_html);
    }

    let html = assemble_html_page(&body_html, &root);

    Ok(html)
}

// ==================== Unified component access layer ====================

/// Unified component access layer
///
/// Abstracts over the `component` and `type` field styles, and over
/// flat fields versus `props`-wrapped structures, providing a unified getter.
struct Component<'a> {
    raw: &'a Value,
}

impl<'a> Component<'a> {
    fn new(raw: &'a Value) -> Self {
        Self { raw }
    }

    /// Get the component type (either the component / type field)
    fn kind(&self) -> &str {
        self.raw
            .get("component")
            .and_then(|v| v.as_str())
            .or_else(|| self.raw.get("type").and_then(|v| v.as_str()))
            .unwrap_or("div")
    }

    /// Get a string attribute (prefers flat fields, then props sub-fields)
    fn str_attr(&self, name: &str) -> Option<&str> {
        self.raw
            .get(name)
            .and_then(|v| v.as_str())
            .or_else(|| {
                self.raw
                    .get("props")
                    .and_then(|p| p.get(name))
                    .and_then(|v| v.as_str())
            })
    }

    /// Get any attribute value (prefers flat fields)
    fn attr(&self, name: &str) -> Option<&Value> {
        self.raw
            .get(name)
            .or_else(|| self.raw.get("props").and_then(|p| p.get(name)))
    }

    /// Get the list of child components
    fn children(&self) -> Vec<Component> {
        self.raw
            .get("children")
            .and_then(|c| c.as_array())
            .map(|arr| arr.iter().map(Component::new).collect())
            .unwrap_or_default()
    }

    /// Get the text content
    fn text(&self) -> String {
        // First try the text field
        if let Some(t) = self.str_attr("text") {
            return t.to_string();
        }
        // Then try string children in the children array
        if let Some(children) = self.raw.get("children").and_then(|c| c.as_array()) {
            let mut texts = Vec::new();
            for child in children {
                if let Some(s) = child.as_str() {
                    texts.push(s.to_string());
                }
            }
            if !texts.is_empty() {
                return texts.join("");
            }
        }
        String::new()
    }
}

// ==================== Rendering engine ====================

/// Recursively render a component to HTML
fn render_component(component: &Component) -> Result<String> {
    let kind = component.kind();
    let mut html = String::new();

    // Handle special components
    match kind {
        "raw" => {
            // raw outputs the raw HTML directly
            if let Some(content) = component.str_attr("content") {
                html.push_str(content);
            }
            return Ok(html);
        }
        "html" => {
            // html wraps the raw HTML directly
            if let Some(content) = component.str_attr("content") {
                html.push_str(content);
            }
            return Ok(html);
        }
        "page" => {
            return render_page(component);
        }
        "body" => {
            // body renders child components directly
            for child in component.children() {
                html.push_str(&render_component(&child)?);
            }
            return Ok(html);
        }
        "text" | "paragraph" => {
            return render_text(component);
        }
        "heading" => {
            return render_heading(component);
        }
        "image" | "img" => {
            return render_image(component);
        }
        "icon" => {
            return render_icon(component);
        }
        "button" => {
            return render_button(component);
        }
        "input" => {
            return render_input(component);
        }
        "textarea" => {
            return render_textarea(component);
        }
        "select" => {
            return render_select(component);
        }
        "checkbox" => {
            return render_checkbox(component);
        }
        "radio" => {
            return render_radio(component);
        }
        "radio-group" => {
            return render_radio_group(component);
        }
        "table" => {
            return render_table(component);
        }
        "card" => {
            return render_card(component);
        }
        "alert" => {
            return render_alert(component);
        }
        "modal" => {
            return render_modal(component);
        }
        "navbar" => {
            return render_navbar(component);
        }
        "container" => {
            return render_container(component);
        }
        "row" => {
            return render_row(component);
        }
        "col" => {
            return render_col(component);
        }
        "badge" => {
            return render_badge(component);
        }
        "progress" => {
            return render_progress(component);
        }
        "spinner" => {
            return render_spinner(component);
        }
        "tabs" => {
            return render_tabs(component);
        }
        "accordion" => {
            return render_accordion(component);
        }
        "carousel" => {
            return render_carousel(component);
        }
        "dropdown" => {
            return render_dropdown(component);
        }
        "list" | "list-group" => {
            return render_list_group(component);
        }
        "pagination" => {
            return render_pagination(component);
        }
        "breadcrumb" => {
            return render_breadcrumb(component);
        }
        "tooltip" => {
            return render_tooltip(component);
        }
        "popover" => {
            return render_popover(component);
        }
        "toast" => {
            return render_toast(component);
        }
        "offcanvas" => {
            return render_offcanvas(component);
        }
        "form" => {
            return render_form(component);
        }
        "nav" => {
            return render_nav(component);
        }
        "footer" => {
            return render_footer(component);
        }
        "header" => {
            return render_header(component);
        }
        "sidebar" => {
            return render_sidebar(component);
        }
        "grid" => {
            return render_grid(component);
        }
        "divider" | "separator" => {
            return render_divider(component);
        }
        "spacer" => {
            return render_spacer(component);
        }
        "code" => {
            return render_code(component);
        }
        _ => {
            // Unknown component: render as a generic div
            return render_generic(kind, component);
        }
    }
}

/// Render the page component: a complete HTML page skeleton
fn render_page(component: &Component) -> Result<String> {
    let title = component.str_attr("title").unwrap_or("Nefu App");
    let lang = component.str_attr("lang").unwrap_or("zh-CN");
    let class_val = component.str_attr("class").unwrap_or("");

    // Render child components
    let mut body_content = String::new();
    for child in component.children() {
        body_content.push_str(&render_component(&child)?);
    }

    // If child components are empty, try rendering the body child component
    if body_content.is_empty() {
        if let Some(body_comp) = component.attr("body") {
            let comp = Component::new(body_comp);
            for child in comp.children() {
                body_content.push_str(&render_component(&child)?);
            }
        }
    }

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="{}">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
    <link rel="stylesheet" href="{}">
    <link rel="stylesheet" href="{}">
    <link href="{}" rel="stylesheet">
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: 'Inter', -apple-system, sans-serif; }}
        #app {{ min-height: 100vh; }}
    </style>
</head>
<body>
    <div id="app" class="{}">
        {}
    </div>
    <script src="{}"></script>
</body>
</html>"#,
        lang,
        title,
        BOOTSTRAP_CSS_CDN,
        FONTAWESOME_CDN,
        GOOGLE_FONTS_URL,
        class_val,
        body_content,
        BOOTSTRAP_JS_CDN,
    );

    Ok(html)
}

/// Render a generic div container
fn render_generic(kind: &str, component: &Component) -> Result<String> {
    let class_val = component.str_attr("class").unwrap_or("");
    let id = component.str_attr("id").unwrap_or("");
    let style = component.str_attr("style").unwrap_or("");
    let onclick = component.str_attr("onclick").unwrap_or("");

    let mut attrs = String::new();
    if !class_val.is_empty() {
        attrs.push_str(&format!(" class=\"{}\"", class_val));
    }
    if !id.is_empty() {
        attrs.push_str(&format!(" id=\"{}\"", id));
    }
    if !style.is_empty() {
        attrs.push_str(&format!(" style=\"{}\"", style));
    }
    if !onclick.is_empty() {
        attrs.push_str(&format!(" onclick=\"{}\"", onclick));
    }
    // Preserve data-* attributes
    if let Some(props) = component.raw.get("props").and_then(|p| p.as_object()) {
        for (key, val) in props {
            if key.starts_with("data-") {
                if let Some(s) = val.as_str() {
                    attrs.push_str(&format!(" {}=\"{}\"", key, s));
                }
            }
        }
    }

    let text = component.text();
    let mut children_html = String::new();
    for child in component.children() {
        children_html.push_str(&render_component(&child)?);
    }

    Ok(format!(
        "<{}{}>{}{}</{}>",
        kind, attrs, text, children_html, kind
    ))
}

// ==================== Component render functions ====================

fn render_container(component: &Component) -> Result<String> {
    let class_val = component.str_attr("class").unwrap_or("container");
    let mut children_html = String::new();
    for child in component.children() {
        children_html.push_str(&render_component(&child)?);
    }
    Ok(format!("<div class=\"{}\">{}</div>", class_val, children_html))
}

fn render_row(component: &Component) -> Result<String> {
    let class_val = component.str_attr("class").unwrap_or("row");
    let mut children_html = String::new();
    for child in component.children() {
        children_html.push_str(&render_component(&child)?);
    }
    Ok(format!("<div class=\"{}\">{}</div>", class_val, children_html))
}

fn render_col(component: &Component) -> Result<String> {
    let class_val = component.str_attr("class").unwrap_or("col");
    let mut children_html = String::new();
    for child in component.children() {
        children_html.push_str(&render_component(&child)?);
    }
    Ok(format!("<div class=\"{}\">{}</div>", class_val, children_html))
}

fn render_button(component: &Component) -> Result<String> {
    let color = component.str_attr("color").unwrap_or("primary");
    let size = component.str_attr("size").unwrap_or("");
    let outline = component.str_attr("outline").unwrap_or("");
    let class_extra = component.str_attr("class").unwrap_or("");
    let onclick = component.str_attr("onclick").unwrap_or("");

    let mut classes = vec!["btn".to_string()];
    if outline == "true" || outline == "yes" {
        classes.push(format!("btn-outline-{}", color));
    } else {
        classes.push(format!("btn-{}", color));
    }
    if !size.is_empty() {
        classes.push(format!("btn-{}", size));
    }
    if !class_extra.is_empty() {
        classes.push(class_extra.to_string());
    }

    let mut attrs = String::new();
    if !onclick.is_empty() {
        attrs.push_str(&format!(" onclick=\"{}\"", onclick));
    }

    // Handle data-* attributes
    if let Some(props) = component.raw.get("props").and_then(|p| p.as_object()) {
        for (key, val) in props {
            if key.starts_with("data-") {
                if let Some(s) = val.as_str() {
                    attrs.push_str(&format!(" {}=\"{}\"", key, s));
                }
            }
        }
    }

    let text = component.text();
    let mut children_html = String::new();
    for child in component.children() {
        children_html.push_str(&render_component(&child)?);
    }

    Ok(format!(
        "<button class=\"{}\"{}>{}{}</button>",
        classes.join(" "),
        attrs,
        text,
        children_html
    ))
}

fn render_input(component: &Component) -> Result<String> {
    let input_type = component.str_attr("type").unwrap_or("text");
    let placeholder = component.str_attr("placeholder").unwrap_or("");
    let value = component.str_attr("value").unwrap_or("");
    let class_extra = component.str_attr("class").unwrap_or("");
    let label = component.str_attr("label").unwrap_or("");

    let mut classes = vec!["form-control".to_string()];
    if !class_extra.is_empty() {
        classes.push(class_extra.to_string());
    }

    let mut html = String::new();
    if !label.is_empty() {
        html.push_str(&format!("<label class=\"form-label\">{}</label>", label));
    }
    html.push_str(&format!(
        "<input type=\"{}\" class=\"{}\" placeholder=\"{}\" value=\"{}\">",
        input_type,
        classes.join(" "),
        placeholder,
        value
    ));

    Ok(html)
}

fn render_textarea(component: &Component) -> Result<String> {
    let placeholder = component.str_attr("placeholder").unwrap_or("");
    let rows = component.str_attr("rows").unwrap_or("3");
    let class_extra = component.str_attr("class").unwrap_or("");
    let label = component.str_attr("label").unwrap_or("");
    let text = component.text();

    let mut classes = vec!["form-control".to_string()];
    if !class_extra.is_empty() {
        classes.push(class_extra.to_string());
    }

    let mut html = String::new();
    if !label.is_empty() {
        html.push_str(&format!("<label class=\"form-label\">{}</label>", label));
    }
    html.push_str(&format!(
        "<textarea class=\"{}\" placeholder=\"{}\" rows=\"{}\">{}</textarea>",
        classes.join(" "),
        placeholder,
        rows,
        text
    ));

    Ok(html)
}

fn render_select(component: &Component) -> Result<String> {
    let class_extra = component.str_attr("class").unwrap_or("");
    let label = component.str_attr("label").unwrap_or("");

    let mut classes = vec!["form-select".to_string()];
    if !class_extra.is_empty() {
        classes.push(class_extra.to_string());
    }

    let mut html = String::new();
    if !label.is_empty() {
        html.push_str(&format!("<label class=\"form-label\">{}</label>", label));
    }
    html.push_str(&format!("<select class=\"{}\">", classes.join(" ")));

    // Render option child components
    for child in component.children() {
        if child.kind() == "option" {
            let value = child.str_attr("value").unwrap_or("");
            let text = child.text();
            html.push_str(&format!("<option value=\"{}\">{}</option>", value, text));
        }
    }

    html.push_str("</select>");
    Ok(html)
}

fn render_checkbox(component: &Component) -> Result<String> {
    let checked = component.str_attr("checked").unwrap_or("");
    let label = component.str_attr("label").unwrap_or("");
    let id = component.str_attr("id").unwrap_or("");

    let checked_attr = if checked == "true" || checked == "yes" { " checked" } else { "" };
    let id_attr = if !id.is_empty() { format!(" id=\"{}\"", id) } else { String::new() };

    Ok(format!(
        "<div class=\"form-check\"><input class=\"form-check-input\" type=\"checkbox\"{}{}><label class=\"form-check-label\"{}>{}</label></div>",
        id_attr, checked_attr, if !id.is_empty() { format!(" for=\"{}\"", id) } else { String::new() }, label
    ))
}

fn render_radio(component: &Component) -> Result<String> {
    let name = component.str_attr("name").unwrap_or("");
    let value = component.str_attr("value").unwrap_or("");
    let checked = component.str_attr("checked").unwrap_or("");
    let label = component.str_attr("label").unwrap_or("");
    let id = component.str_attr("id").unwrap_or("");

    let checked_attr = if checked == "true" || checked == "yes" { " checked" } else { "" };
    let id_attr = if !id.is_empty() { format!(" id=\"{}\"", id) } else { String::new() };

    Ok(format!(
        "<div class=\"form-check\"><input class=\"form-check-input\" type=\"radio\" name=\"{}\" value=\"{}\"{}{}><label class=\"form-check-label\"{}>{}</label></div>",
        name, value, id_attr, checked_attr, if !id.is_empty() { format!(" for=\"{}\"", id) } else { String::new() }, label
    ))
}

fn render_radio_group(component: &Component) -> Result<String> {
    let label = component.str_attr("label").unwrap_or("");
    let mut html = String::new();
    if !label.is_empty() {
        html.push_str(&format!("<label class=\"form-label\">{}</label>", label));
    }
    for child in component.children() {
        html.push_str(&render_component(&child)?);
    }
    Ok(html)
}

fn render_text(component: &Component) -> Result<String> {
    let tag = component.str_attr("tag").unwrap_or("p");
    let class_extra = component.str_attr("class").unwrap_or("");
    let text = component.text();
    let mut children_html = String::new();
    for child in component.children() {
        children_html.push_str(&render_component(&child)?);
    }

    let class_attr = if !class_extra.is_empty() {
        format!(" class=\"{}\"", class_extra)
    } else {
        String::new()
    };

    Ok(format!(
        "<{}{}>{}{}</{}>",
        tag, class_attr, text, children_html, tag
    ))
}

fn render_heading(component: &Component) -> Result<String> {
    let level = component.str_attr("level").unwrap_or("2");
    let class_extra = component.str_attr("class").unwrap_or("");
    let text = component.text();

    let class_attr = if !class_extra.is_empty() {
        format!(" class=\"{}\"", class_extra)
    } else {
        String::new()
    };

    Ok(format!(
        "<h{}{}>{}</h{}>",
        level, class_attr, text, level
    ))
}

fn render_image(component: &Component) -> Result<String> {
    let src = component.str_attr("src").unwrap_or("");
    let alt = component.str_attr("alt").unwrap_or("");
    let class_extra = component.str_attr("class").unwrap_or("img-fluid");
    let width = component.str_attr("width").unwrap_or("");
    let height = component.str_attr("height").unwrap_or("");

    let mut attrs = String::new();
    if !class_extra.is_empty() {
        attrs.push_str(&format!(" class=\"{}\"", class_extra));
    }
    if !width.is_empty() {
        attrs.push_str(&format!(" width=\"{}\"", width));
    }
    if !height.is_empty() {
        attrs.push_str(&format!(" height=\"{}\"", height));
    }

    Ok(format!(
        "<img src=\"{}\" alt=\"{}\"{}>",
        src, alt, attrs
    ))
}

fn render_icon(component: &Component) -> Result<String> {
    let name = component.str_attr("name").unwrap_or("circle");
    let class_extra = component.str_attr("class").unwrap_or("");

    let mut classes = vec![format!("fa-{}", name)];
    if !class_extra.is_empty() {
        classes.push(class_extra.to_string());
    }

    Ok(format!("<i class=\"{}\"></i>", classes.join(" ")))
}

fn render_card(component: &Component) -> Result<String> {
    let class_extra = component.str_attr("class").unwrap_or("");

    let mut classes = vec!["card".to_string()];
    if !class_extra.is_empty() {
        classes.push(class_extra.to_string());
    }

    let mut html = format!("<div class=\"{}\">", classes.join(" "));

    // Render child components
    for child in component.children() {
        html.push_str(&render_component(&child)?);
    }

    html.push_str("</div>");
    Ok(html)
}

fn render_alert(component: &Component) -> Result<String> {
    let color = component.str_attr("color").unwrap_or("info");
    let dismissible = component.str_attr("dismissible").unwrap_or("");
    let class_extra = component.str_attr("class").unwrap_or("");
    let text = component.text();

    let mut classes = vec![format!("alert alert-{}", color)];
    if dismissible == "true" || dismissible == "yes" {
        classes.push("alert-dismissible fade show".to_string());
    }
    if !class_extra.is_empty() {
        classes.push(class_extra.to_string());
    }

    let mut html = format!("<div class=\"{}\" role=\"alert\">", classes.join(" "));
    html.push_str(&text);
    if dismissible == "true" || dismissible == "yes" {
        html.push_str("<button type=\"button\" class=\"btn-close\" data-bs-dismiss=\"alert\"></button>");
    }
    html.push_str("</div>");

    Ok(html)
}

fn render_badge(component: &Component) -> Result<String> {
    let color = component.str_attr("color").unwrap_or("secondary");
    let class_extra = component.str_attr("class").unwrap_or("");
    let text = component.text();

    let mut classes = vec![format!("badge bg-{}", color)];
    if !class_extra.is_empty() {
        classes.push(class_extra.to_string());
    }

    Ok(format!("<span class=\"{}\">{}</span>", classes.join(" "), text))
}

fn render_progress(component: &Component) -> Result<String> {
    let value = component.str_attr("value").unwrap_or("0");
    let max = component.str_attr("max").unwrap_or("100");
    let color = component.str_attr("color").unwrap_or("");
    let striped = component.str_attr("striped").unwrap_or("");
    let animated = component.str_attr("animated").unwrap_or("");
    let label = component.str_attr("label").unwrap_or("");

    let mut bar_classes = vec!["progress-bar".to_string()];
    if !color.is_empty() {
        bar_classes.push(format!("bg-{}", color));
    }
    if striped == "true" || striped == "yes" {
        bar_classes.push("progress-bar-striped".to_string());
    }
    if animated == "true" || animated == "yes" {
        bar_classes.push("progress-bar-animated".to_string());
    }

    let percent = if let (Ok(v), Ok(m)) = (value.parse::<f64>(), max.parse::<f64>()) {
        if m > 0.0 { (v / m * 100.0) as u32 } else { 0 }
    } else {
        0
    };

    let label_text = if !label.is_empty() { label.to_string() } else { format!("{}%", percent) };

    Ok(format!(
        "<div class=\"progress\"><div class=\"{}\" role=\"progressbar\" style=\"width: {}%\" aria-valuenow=\"{}\" aria-valuemin=\"0\" aria-valuemax=\"{}\">{}</div></div>",
        bar_classes.join(" "),
        percent,
        value,
        max,
        label_text
    ))
}

fn render_spinner(component: &Component) -> Result<String> {
    let spinner_type = component.str_attr("type").unwrap_or("border");
    let color = component.str_attr("color").unwrap_or("primary");
    let size = component.str_attr("size").unwrap_or("");
    let class_extra = component.str_attr("class").unwrap_or("");

    let mut classes = vec![format!("spinner-{} text-{}", spinner_type, color)];
    if !size.is_empty() {
        classes.push(format!("spinner-{}-{}", spinner_type, size));
    }
    if !class_extra.is_empty() {
        classes.push(class_extra.to_string());
    }

    let role = if spinner_type == "border" { "status" } else { "status" };

    Ok(format!(
        "<div class=\"{}\" role=\"{}\"><span class=\"visually-hidden\">Loading...</span></div>",
        classes.join(" "),
        role
    ))
}

fn render_modal(component: &Component) -> Result<String> {
    let id = component.str_attr("id").unwrap_or("modal");
    let title = component.str_attr("title").unwrap_or("Title");
    let size = component.str_attr("size").unwrap_or("");
    let class_extra = component.str_attr("class").unwrap_or("");

    let mut classes = vec!["modal-dialog".to_string()];
    if !size.is_empty() {
        classes.push(format!("modal-{}", size));
    }
    if !class_extra.is_empty() {
        classes.push(class_extra.to_string());
    }

    let mut html = format!(
        r#"<div class="modal fade" id="{}" tabindex="-1">
  <div class="{}">
    <div class="modal-content">
      <div class="modal-header">
        <h5 class="modal-title">{}</h5>
        <button type="button" class="btn-close" data-bs-dismiss="modal"></button>
      </div>
      <div class="modal-body">"#,
        id,
        classes.join(" "),
        title
    );

    for child in component.children() {
        html.push_str(&render_component(&child)?);
    }

    html.push_str(r#"</div></div></div></div>"#);
    Ok(html)
}

fn render_navbar(component: &Component) -> Result<String> {
    let brand = component.str_attr("brand").unwrap_or("");
    let color = component.str_attr("color").unwrap_or("light");
    let bg = component.str_attr("bg").unwrap_or("light");
    let fixed = component.str_attr("fixed").unwrap_or("");
    let class_extra = component.str_attr("class").unwrap_or("");

    let mut classes = vec![format!("navbar navbar-expand-lg navbar-{} bg-{}", color, bg)];
    if !fixed.is_empty() {
        classes.push(format!("fixed-{}", fixed));
    }
    if !class_extra.is_empty() {
        classes.push(class_extra.to_string());
    }

    let mut html = format!("<nav class=\"{}\"><div class=\"container-fluid\">", classes.join(" "));
    if !brand.is_empty() {
        html.push_str(&format!("<a class=\"navbar-brand\" href=\"#\">{}</a>", brand));
    }

    // Render child components
    for child in component.children() {
        html.push_str(&render_component(&child)?);
    }

    html.push_str("</div></nav>");
    Ok(html)
}

fn render_tabs(component: &Component) -> Result<String> {
    let id = component.str_attr("id").unwrap_or("tabs");
    let mut html = format!("<ul class=\"nav nav-tabs\" id=\"{}\" role=\"tablist\">", id);

    // The first child component is the default active tab
    for (i, child) in component.children().iter().enumerate() {
        let title_str = format!("Tab {}", i + 1);
        let title = child.str_attr("title").unwrap_or(&title_str);
        let active = if i == 0 { " active" } else { "" };
        let target = format!("#{}-{}", id, i);
        html.push_str(&format!(
            r#"<li class="nav-item" role="presentation">
                <button class="nav-link{0}" id="{1}-tab-{2}" data-bs-toggle="tab" data-bs-target="{3}" type="button" role="tab">{4}</button>
              </li>"#,
            active, id, i, target, title
        ));
    }

    html.push_str("</ul><div class=\"tab-content\">");
    for (i, child) in component.children().iter().enumerate() {
        let active = if i == 0 { " show active" } else { "" };
        html.push_str(&format!(
            "<div class=\"tab-pane fade{}\" id=\"{}-{}\" role=\"tabpanel\">",
            active, id, i
        ));
        html.push_str(&render_component(child)?);
        html.push_str("</div>");
    }

    html.push_str("</div>");
    Ok(html)
}

fn render_accordion(component: &Component) -> Result<String> {
    let id = component.str_attr("id").unwrap_or("accordion");
    let mut html = format!("<div class=\"accordion\" id=\"{}\">", id);

    for (i, child) in component.children().iter().enumerate() {
        let title_str = format!("Item {}", i + 1);
        let title = child.str_attr("title").unwrap_or(&title_str);
        let show = if i == 0 { " show" } else { "" };
        let collapsed = if i == 0 { "" } else { " collapsed" };

        let target = format!("#{}-collapse-{}", id, i);
        let parent = format!("#{}", id);
        html.push_str(&format!(
            r##"<div class="accordion-item">
                <h2 class="accordion-header">
                  <button class="accordion-button{0}" type="button" data-bs-toggle="collapse" data-bs-target="{1}">{2}</button>
                </h2>
                <div id="{3}-collapse-{4}" class="accordion-collapse collapse{5}" data-bs-parent="{6}">
                  <div class="accordion-body"></div></div></div>"##,
            collapsed, target, title, id, i, show, parent
        ));
    }

    html.push_str("</div>");
    Ok(html)
}

fn render_carousel(component: &Component) -> Result<String> {
    let id = component.str_attr("id").unwrap_or("carousel");
    let mut html = format!("<div id=\"{}\" class=\"carousel slide\" data-bs-ride=\"carousel\">", id);

    // Indicators
    let children = component.children();
    html.push_str("<div class=\"carousel-indicators\">");
    for (i, _) in children.iter().enumerate() {
        let active = if i == 0 { " class=\"active\"" } else { "" };
        html.push_str(&format!(
            "<button type=\"button\" data-bs-target=\"#{}\" data-bs-slide-to=\"{}\"{}</button>",
            id, i, active
        ));
    }
    html.push_str("</div>");

    html.push_str("<div class=\"carousel-inner\">");
    for (i, child) in children.iter().enumerate() {
        let active = if i == 0 { " active" } else { "" };
        html.push_str(&format!("<div class=\"carousel-item{}\">", active));
        html.push_str(&render_component(child)?);
        html.push_str("</div>");
    }
    html.push_str("</div>");

    html.push_str(&format!(
        r##"<button class="carousel-control-prev" type="button" data-bs-target="#{}" data-bs-slide="prev">
            <span class="carousel-control-prev-icon"></span>
          </button>
          <button class="carousel-control-next" type="button" data-bs-target="#{}" data-bs-slide="next">
            <span class="carousel-control-next-icon"></span>
          </button>"##,
        id, id
    ));

    html.push_str("</div>");
    Ok(html)
}

fn render_table(component: &Component) -> Result<String> {
    let bordered = component.str_attr("bordered").unwrap_or("");
    let striped = component.str_attr("striped").unwrap_or("");
    let hover = component.str_attr("hover").unwrap_or("");
    let class_extra = component.str_attr("class").unwrap_or("");

    let mut classes = vec!["table".to_string()];
    if bordered == "true" || bordered == "yes" {
        classes.push("table-bordered".to_string());
    }
    if striped == "true" || striped == "yes" {
        classes.push("table-striped".to_string());
    }
    if hover == "true" || hover == "yes" {
        classes.push("table-hover".to_string());
    }
    if !class_extra.is_empty() {
        classes.push(class_extra.to_string());
    }

    let mut html = format!("<table class=\"{}\">", classes.join(" "));

    for child in component.children() {
        match child.kind() {
            "thead" | "header" => {
                html.push_str("<thead>");
                for row in child.children() {
                    html.push_str("<tr>");
                    for cell in row.children() {
                        html.push_str(&format!("<th>{}</th>", cell.text()));
                    }
                    html.push_str("</tr>");
                }
                html.push_str("</thead>");
            }
            "tbody" | "body" => {
                html.push_str("<tbody>");
                for row in child.children() {
                    html.push_str("<tr>");
                    for cell in row.children() {
                        html.push_str(&format!("<td>{}</td>", cell.text()));
                    }
                    html.push_str("</tr>");
                }
                html.push_str("</tbody>");
            }
            _ => {
                html.push_str(&render_component(&child)?);
            }
        }
    }

    html.push_str("</table>");
    Ok(html)
}

fn render_dropdown(component: &Component) -> Result<String> {
    let text = component.text();
    let color = component.str_attr("color").unwrap_or("secondary");
    let class_extra = component.str_attr("class").unwrap_or("");

    let mut classes = vec!["dropdown".to_string()];
    if !class_extra.is_empty() {
        classes.push(class_extra.to_string());
    }

    let mut html = format!("<div class=\"{}\">", classes.join(" "));
    html.push_str(&format!(
        r#"<button class="btn btn-{} dropdown-toggle" type="button" data-bs-toggle="dropdown">{}</button>
           <ul class="dropdown-menu">"#,
        color, text
    ));

    for child in component.children() {
        if child.kind() == "divider" {
            html.push_str("<li><hr class=\"dropdown-divider\"></li>");
        } else {
            let href = child.str_attr("href").unwrap_or("#");
            let item_text = child.text();
            html.push_str(&format!("<li><a class=\"dropdown-item\" href=\"{}\">{}</a></li>", href, item_text));
        }
    }

    html.push_str("</ul></div>");
    Ok(html)
}

fn render_list_group(component: &Component) -> Result<String> {
    let flush = component.str_attr("flush").unwrap_or("");
    let numbered = component.str_attr("numbered").unwrap_or("");
    let class_extra = component.str_attr("class").unwrap_or("");

    let mut classes = vec!["list-group".to_string()];
    if flush == "true" || flush == "yes" {
        classes.push("list-group-flush".to_string());
    }
    if numbered == "true" || numbered == "yes" {
        classes.push("list-group-numbered".to_string());
    }
    if !class_extra.is_empty() {
        classes.push(class_extra.to_string());
    }

    let mut html = format!("<ul class=\"{}\">", classes.join(" "));

    for child in component.children() {
        let active = child.str_attr("active").unwrap_or("") == "true";
        let disabled = child.str_attr("disabled").unwrap_or("") == "true";
        let item_class = child.str_attr("class").unwrap_or("");
        let text = child.text();

        let mut item_classes = vec!["list-group-item".to_string()];
        if active { item_classes.push("active".to_string()); }
        if disabled { item_classes.push("disabled".to_string()); }
        if !item_class.is_empty() { item_classes.push(item_class.to_string()); }

        html.push_str(&format!("<li class=\"{}\">{}</li>", item_classes.join(" "), text));
    }

    html.push_str("</ul>");
    Ok(html)
}

fn render_pagination(component: &Component) -> Result<String> {
    let size = component.str_attr("size").unwrap_or("");
    let class_extra = component.str_attr("class").unwrap_or("");

    let mut classes = vec!["pagination".to_string()];
    if !size.is_empty() {
        classes.push(format!("pagination-{}", size));
    }
    if !class_extra.is_empty() {
        classes.push(class_extra.to_string());
    }

    let mut html = format!("<nav><ul class=\"{}\">", classes.join(" "));

    for child in component.children() {
        let active = child.str_attr("active").unwrap_or("") == "true";
        let disabled = child.str_attr("disabled").unwrap_or("") == "true";
        let text = child.text();

        let mut item_classes = vec!["page-item".to_string()];
        if active { item_classes.push("active".to_string()); }
        if disabled { item_classes.push("disabled".to_string()); }

        html.push_str(&format!(
            "<li class=\"{}\"><a class=\"page-link\" href=\"#\">{}</a></li>",
            item_classes.join(" "),
            text
        ));
    }

    html.push_str("</ul></nav>");
    Ok(html)
}

fn render_breadcrumb(component: &Component) -> Result<String> {
    let mut html = String::from("<nav><ol class=\"breadcrumb\">");

    for child in component.children() {
        let active = child.str_attr("active").unwrap_or("") == "true";
        let text = child.text();
        let href = child.str_attr("href").unwrap_or("#");

        if active {
            html.push_str(&format!("<li class=\"breadcrumb-item active\">{}</li>", text));
        } else {
            html.push_str(&format!("<li class=\"breadcrumb-item\"><a href=\"{}\">{}</a></li>", href, text));
        }
    }

    html.push_str("</ol></nav>");
    Ok(html)
}

fn render_tooltip(component: &Component) -> Result<String> {
    let title = component.str_attr("title").unwrap_or("");
    let placement = component.str_attr("placement").unwrap_or("top");
    let text = component.text();
    let class_extra = component.str_attr("class").unwrap_or("");

    Ok(format!(
        "<span class=\"{}\" data-bs-toggle=\"tooltip\" data-bs-placement=\"{}\" title=\"{}\">{}</span>",
        class_extra, placement, title, text
    ))
}

fn render_popover(component: &Component) -> Result<String> {
    let title = component.str_attr("title").unwrap_or("");
    let content = component.str_attr("content").unwrap_or("");
    let placement = component.str_attr("placement").unwrap_or("right");
    let text = component.text();
    let class_extra = component.str_attr("class").unwrap_or("");

    Ok(format!(
        "<span class=\"{}\" data-bs-toggle=\"popover\" data-bs-placement=\"{}\" title=\"{}\" data-bs-content=\"{}\">{}</span>",
        class_extra, placement, title, content, text
    ))
}

fn render_toast(component: &Component) -> Result<String> {
    let title = component.str_attr("title").unwrap_or("Notification");
    let time = component.str_attr("time").unwrap_or("Just now");
    let class_extra = component.str_attr("class").unwrap_or("");

    let mut html = format!(
        r#"<div class="toast {}" role="alert">
            <div class="toast-header">
              <strong class="me-auto">{}</strong>
              <small>{}</small>
              <button type="button" class="btn-close" data-bs-dismiss="toast"></button>
            </div>
            <div class="toast-body">"#,
        class_extra, title, time
    );

    for child in component.children() {
        html.push_str(&render_component(&child)?);
    }

    html.push_str("</div></div>");
    Ok(html)
}

fn render_offcanvas(component: &Component) -> Result<String> {
    let id = component.str_attr("id").unwrap_or("offcanvas");
    let title = component.str_attr("title").unwrap_or("Title");
    let placement = component.str_attr("placement").unwrap_or("start");
    let class_extra = component.str_attr("class").unwrap_or("");

    let mut html = format!(
        r#"<div class="offcanvas offcanvas-{} {}" tabindex="-1" id="{}">
            <div class="offcanvas-header">
              <h5 class="offcanvas-title">{}</h5>
              <button type="button" class="btn-close" data-bs-dismiss="offcanvas"></button>
            </div>
            <div class="offcanvas-body">"#,
        placement, class_extra, id, title
    );

    for child in component.children() {
        html.push_str(&render_component(&child)?);
    }

    html.push_str("</div></div>");
    Ok(html)
}

fn render_form(component: &Component) -> Result<String> {
    let action = component.str_attr("action").unwrap_or("");
    let method = component.str_attr("method").unwrap_or("post");
    let class_extra = component.str_attr("class").unwrap_or("");

    let mut html = format!(
        "<form action=\"{}\" method=\"{}\" class=\"{}\">",
        action, method, class_extra
    );

    for child in component.children() {
        html.push_str(&render_component(&child)?);
    }

    html.push_str("</form>");
    Ok(html)
}

fn render_nav(component: &Component) -> Result<String> {
    let class_extra = component.str_attr("class").unwrap_or("nav");
    let mut html = format!("<ul class=\"{}\">", class_extra);

    for child in component.children() {
        let active = child.str_attr("active").unwrap_or("") == "true";
        let href = child.str_attr("href").unwrap_or("#");
        let text = child.text();

        let mut classes = vec!["nav-item".to_string()];
        if active { classes.push("active".to_string()); }

        html.push_str(&format!(
            "<li class=\"{}\"><a class=\"nav-link\" href=\"{}\">{}</a></li>",
            classes.join(" "), href, text
        ));
    }

    html.push_str("</ul>");
    Ok(html)
}

fn render_footer(component: &Component) -> Result<String> {
    let class_extra = component.str_attr("class").unwrap_or("footer");
    let mut children_html = String::new();
    for child in component.children() {
        children_html.push_str(&render_component(&child)?);
    }
    Ok(format!("<footer class=\"{}\">{}</footer>", class_extra, children_html))
}

fn render_header(component: &Component) -> Result<String> {
    let class_extra = component.str_attr("class").unwrap_or("header");
    let mut children_html = String::new();
    for child in component.children() {
        children_html.push_str(&render_component(&child)?);
    }
    Ok(format!("<header class=\"{}\">{}</header>", class_extra, children_html))
}

fn render_sidebar(component: &Component) -> Result<String> {
    let class_extra = component.str_attr("class").unwrap_or("sidebar");
    let mut children_html = String::new();
    for child in component.children() {
        children_html.push_str(&render_component(&child)?);
    }
    Ok(format!("<aside class=\"{}\">{}</aside>", class_extra, children_html))
}

fn render_grid(component: &Component) -> Result<String> {
    let cols = component.str_attr("cols").unwrap_or("3");
    let gap = component.str_attr("gap").unwrap_or("3");
    let class_extra = component.str_attr("class").unwrap_or("");

    let mut classes = vec![format!("row row-cols-{}", cols)];
    if !gap.is_empty() {
        classes.push(format!("g-{}", gap));
    }
    if !class_extra.is_empty() {
        classes.push(class_extra.to_string());
    }

    let mut children_html = String::new();
    for child in component.children() {
        children_html.push_str(&format!("<div class=\"col\">{}</div>", &render_component(&child)?));
    }

    Ok(format!("<div class=\"{}\">{}</div>", classes.join(" "), children_html))
}

fn render_divider(component: &Component) -> Result<String> {
    let class_extra = component.str_attr("class").unwrap_or("");
    Ok(format!("<hr class=\"{}\">", class_extra))
}

fn render_spacer(component: &Component) -> Result<String> {
    let size = component.str_attr("size").unwrap_or("3");
    Ok(format!("<div style=\"height: {}rem\"></div>", size))
}

fn render_code(component: &Component) -> Result<String> {
    let language = component.str_attr("language").unwrap_or("");
    let inline = component.str_attr("inline").unwrap_or("");
    let text = component.text();

    if inline == "true" || inline == "yes" {
        Ok(format!("<code>{}</code>", text))
    } else {
        let class_attr = if !language.is_empty() {
            format!(" class=\"language-{}\"", language)
        } else {
            String::new()
        };
        Ok(format!("<pre><code{}>{}</code></pre>", class_attr, text))
    }
}

// ==================== HTML assembly ====================

/// Assemble a complete HTML page
fn assemble_html_page(body_html: &str, root: &Value) -> String {
    let title = root
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Nefu App");

    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
    <link rel="stylesheet" href="{}">
    <link rel="stylesheet" href="{}">
    <link href="{}" rel="stylesheet">
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: 'Inter', -apple-system, sans-serif; }}
        #app {{ min-height: 100vh; }}
    </style>
</head>
<body>
    <div id="app">
        {}
    </div>
    <script src="{}"></script>
</body>
</html>"#,
        title,
        BOOTSTRAP_CSS_CDN,
        FONTAWESOME_CDN,
        GOOGLE_FONTS_URL,
        body_html,
        BOOTSTRAP_JS_CDN,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_comments() {
        let input = "\\ This is a comment\nimport<nefu.nch>\n/* multi-line\ncomment */\nmain\"";
        let result = remove_comments(input);
        assert!(!result.contains("This is a comment"));
        assert!(result.contains("import<nefu.nch>"));
        assert!(!result.contains("multi-line"));
        assert!(result.contains("main\""));
    }

    #[test]
    fn test_parse_nefu_call() {
        let result = parse_nefu_call("nefu(\"list\",[1,2,3,4],BLACK)").unwrap();
        assert_eq!(result, "await nefu.invoke(\"list\", [1, 2, 3, 4], \"BLACK\")");
    }

    #[test]
    fn test_parse_nefu_call_simple() {
        let result = parse_nefu_call("nefu(\"alert\",\"hello\")").unwrap();
        assert_eq!(result, "await nefu.invoke(\"alert\", \"hello\")");
    }

    #[test]
    fn test_is_new_syntax() {
        assert!(is_new_syntax("import<nefu.nch>"));
        assert!(is_new_syntax("main\""));
        assert!(is_new_syntax("\\ comment"));
        assert!(!is_new_syntax("{ \"component\": \"button\" }"));
    }

    #[test]
    fn test_parse_args() {
        let args = parse_nefu_args("\"list\",[1,2,3,4],BLACK");
        assert_eq!(args.len(), 3);
        assert!(matches!(args[0], NefuArg::String(_)));
        assert!(matches!(args[1], NefuArg::Array(_)));
        assert!(matches!(args[2], NefuArg::Identifier(_)));
    }

    #[test]
    fn test_parse_new_syntax_basic() {
        let input = r#"
import<nefu.nch>
\ This is a test
main"
  nefu("alert","Hello World")
  nefu("list",[1,2,3],TEST)
end(all);
"#;
        let result = parse_new_syntax(input).unwrap();
        assert!(result.contains("nefu.invoke"));
        assert!(result.contains("Hello World"));
        assert!(result.contains("TEST"));
        assert!(result.contains("<html"));
    }

    #[test]
    fn test_old_syntax_still_works() {
        let input = r#"{"component":"button","text":"Hello","color":"primary"}"#;
        let result = parse_nc_to_html(input).unwrap();
        assert!(result.contains("btn-primary"));
        assert!(result.contains("Hello"));
        assert!(result.contains("<html"));
    }
}