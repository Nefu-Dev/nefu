// Nefu — 将网页项目打包为桌面可执行文件
// Go 语言实现，支持加密打包、热重载开发、.nc 组件语言
package main

import (
	"archive/zip"
	"bytes"
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"crypto/sha256"
	"encoding/binary"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"time"

	"github.com/BurntSushi/toml"
	"github.com/fsnotify/fsnotify"
)

// ============================================================
// 常量定义
// ============================================================

const (
	// NEFUPACK_MAGIC 打包文件的魔数标识
	NEFUPACK_MAGIC = "NEFUPACK"
	// MAGIC_LEN 魔数长度（8字节）
	MAGIC_LEN = 8
	// HASH_LEN SHA256 哈希长度（32字节）
	HASH_LEN = 32
	// LEN_SIZE 数据长度字段大小（8字节）
	LEN_SIZE = 8
	// TRAILER_SIZE 尾部总大小 = 长度 + 哈希 + 魔数
	TRAILER_SIZE = LEN_SIZE + HASH_LEN + MAGIC_LEN
	// AES_KEY_SIZE AES-256 密钥长度
	AES_KEY_SIZE = 32
	// DEFAULT_PORT 默认开发服务器端口
	DEFAULT_PORT = 3900
	// CONFIG_FILE 配置文件名
	CONFIG_FILE = "main.nefu"
)

// ============================================================
// 配置结构体
// ============================================================

// NefuConfig 主配置结构，对应 main.nefu TOML 文件
type NefuConfig struct {
	App    AppConfig    `toml:"app"`
	Build  BuildConfig  `toml:"build"`
	Dev    DevConfig    `toml:"dev"`
	Window WindowConfig `toml:"window"`
	Tray   TrayConfig   `toml:"tray"`
}

// AppConfig 应用基本信息
type AppConfig struct {
	Name       string `toml:"name"`
	Version    string `toml:"version"`
	Width      int    `toml:"width"`
	Height     int    `toml:"height"`
	Resizable  bool   `toml:"resizable"`
	Fullscreen bool   `toml:"fullscreen"`
	Icon       string `toml:"icon"`
}

// BuildConfig 构建相关配置
type BuildConfig struct {
	Entry    string `toml:"entry"`
	Preload  string `toml:"preload"`
	Output   string `toml:"output"`
	Encrypt  bool   `toml:"encrypt"`
	Compress bool   `toml:"compress"`
}

// DevConfig 开发服务器配置
type DevConfig struct {
	Port  int  `toml:"port"`
	Open  bool `toml:"open"`
	Watch bool `toml:"watch"`
}

// WindowConfig 窗口配置
type WindowConfig struct {
	Title        string `toml:"title"`
	MinWidth     int    `toml:"min_width"`
	MinHeight    int    `toml:"min_height"`
	Center       bool   `toml:"center"`
	Frameless    bool   `toml:"frameless"`
	AlwaysOnTop  bool   `toml:"always_on_top"`
}

// TrayConfig 系统托盘配置
type TrayConfig struct {
	Enabled bool   `toml:"enabled"`
	Icon    string `toml:"icon"`
	Tooltip string `toml:"tooltip"`
}

// ============================================================
// NC 组件结构体（.nc JSON UI 语言）
// ============================================================

// NCComponent 表示一个 .nc UI 组件节点
type NCComponent struct {
	Component   string                 `json:"component"`
	Children    []*NCComponent         `json:"children,omitempty"`
	Props       map[string]interface{} `json:"-"`
	RawJSON     map[string]interface{} `json:"-"`
}

// UnmarshalJSON 自定义反序列化，将所有非 children/component 字段归入 Props
func (nc *NCComponent) UnmarshalJSON(data []byte) error {
	// 先解析所有字段
	var raw map[string]interface{}
	if err := json.Unmarshal(data, &raw); err != nil {
		return fmt.Errorf("解析 .nc 组件失败: %w", err)
	}
	nc.RawJSON = raw

	// 提取 component 名称
	if comp, ok := raw["component"].(string); ok {
		nc.Component = comp
	}

	// 递归解析 children
	if childrenRaw, ok := raw["children"].([]interface{}); ok {
		for _, childRaw := range childrenRaw {
			childBytes, _ := json.Marshal(childRaw)
			child := &NCComponent{}
			if err := child.UnmarshalJSON(childBytes); err != nil {
				return err
			}
			nc.Children = append(nc.Children, child)
		}
	}

	// 其余字段作为 props
	nc.Props = make(map[string]interface{})
	for k, v := range raw {
		if k != "component" && k != "children" {
			nc.Props[k] = v
		}
	}

	return nil
}

// GetProp 安全获取属性值
func (nc *NCComponent) GetProp(key string) interface{} {
	if nc.Props == nil {
		return nil
	}
	return nc.Props[key]
}

// GetPropString 获取字符串属性
func (nc *NCComponent) GetPropString(key string) string {
	v := nc.GetProp(key)
	if s, ok := v.(string); ok {
		return s
	}
	return ""
}

// GetPropBool 获取布尔属性
func (nc *NCComponent) GetPropBool(key string) bool {
	v := nc.GetProp(key)
	if b, ok := v.(bool); ok {
		return b
	}
	return false
}

// GetPropInt 获取整数属性
func (nc *NCComponent) GetPropInt(key string) int {
	v := nc.GetProp(key)
	switch n := v.(type) {
	case float64:
		return int(n)
	case int:
		return n
	default:
		return 0
	}
}

// ============================================================
// NC 组件渲染器
// ============================================================

// NCRenderer 将 .nc 组件树渲染为 HTML
type NCRenderer struct {
	indent int
}

// NewNCRenderer 创建渲染器实例
func NewNCRenderer() *NCRenderer {
	return &NCRenderer{indent: 0}
}

// Render 渲染整个组件树为完整 HTML 页面
func (r *NCRenderer) Render(root *NCComponent) string {
	title := root.GetPropString("title")
	if title == "" {
		title = "Nefu App"
	}

	var buf strings.Builder
	buf.WriteString("<!DOCTYPE html>\n<html lang=\"zh-CN\">\n<head>\n")
	buf.WriteString("  <meta charset=\"UTF-8\">\n")
	buf.WriteString("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n")
	buf.WriteString(fmt.Sprintf("  <title>%s</title>\n", htmlEscape(title)))
	// Bootstrap 5 CSS
	buf.WriteString("  <link href=\"https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/css/bootstrap.min.css\" rel=\"stylesheet\">\n")
	// Bootstrap Icons
	buf.WriteString("  <link href=\"https://cdn.jsdelivr.net/npm/bootstrap-icons@1.11.0/font/bootstrap-icons.css\" rel=\"stylesheet\">\n")
	buf.WriteString("</head>\n<body>\n")

	// 渲染组件树
	r.renderComponent(&buf, root)

	// Bootstrap JS
	buf.WriteString("\n<script src=\"https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/js/bootstrap.bundle.min.js\"></script>\n")
	buf.WriteString("</body>\n</html>")

	return buf.String()
}

// renderComponent 递归渲染单个组件
func (r *NCRenderer) renderComponent(buf *strings.Builder, comp *NCComponent) {
	if comp == nil {
		return
	}

	switch comp.Component {
	case "page":
		r.renderChildren(buf, comp)

	case "navbar":
		r.renderNavbar(buf, comp)

	case "container":
		fluid := comp.GetPropBool("fluid")
		cls := comp.GetPropString("class")
		containerClass := "container"
		if fluid {
			containerClass = "container-fluid"
		}
		if cls != "" {
			containerClass += " " + cls
		}
		fmt.Fprintf(buf, "<div class=\"%s\">\n", containerClass)
		r.renderChildren(buf, comp)
		buf.WriteString("</div>\n")

	case "row":
		cls := comp.GetPropString("class")
		rowClass := "row"
		if cls != "" {
			rowClass += " " + cls
		}
		fmt.Fprintf(buf, "<div class=\"%s\">\n", rowClass)
		r.renderChildren(buf, comp)
		buf.WriteString("</div>\n")

	case "col":
		size := comp.GetPropInt("size")
		cls := comp.GetPropString("class")
		colClass := fmt.Sprintf("col-md-%d", size)
		if cls != "" {
			colClass += " " + cls
		}
		fmt.Fprintf(buf, "<div class=\"%s\">\n", colClass)
		r.renderChildren(buf, comp)
		buf.WriteString("</div>\n")

	case "card":
		r.renderCard(buf, comp)

	case "button":
		r.renderButton(buf, comp)

	case "input":
		r.renderInput(buf, comp)

	case "select":
		r.renderSelect(buf, comp)

	case "textarea":
		r.renderTextarea(buf, comp)

	case "checkbox":
		r.renderCheckbox(buf, comp)

	case "radio-group":
		r.renderRadioGroup(buf, comp)

	case "form":
		r.renderForm(buf, comp)

	case "table":
		r.renderTable(buf, comp)

	case "alert":
		r.renderAlert(buf, comp)

	case "badge":
		r.renderBadge(buf, comp)

	case "progress":
		r.renderProgress(buf, comp)

	case "modal":
		r.renderModal(buf, comp)

	case "tabs":
		r.renderTabs(buf, comp)

	case "accordion":
		r.renderAccordion(buf, comp)

	case "list":
		r.renderList(buf, comp)

	case "list-group":
		r.renderListGroup(buf, comp)

	case "heading":
		level := comp.GetPropInt("level")
		if level < 1 || level > 6 {
			level = 3
		}
		text := comp.GetPropString("text")
		cls := comp.GetPropString("class")
		classAttr := ""
		if cls != "" {
			classAttr = fmt.Sprintf(" class=\"%s\"", htmlEscape(cls))
		}
		fmt.Fprintf(buf, "<h%d%s>%s</h%d>\n", level, classAttr, htmlEscape(text), level)

	case "text":
		content := comp.GetPropString("content")
		cls := comp.GetPropString("class")
		if cls != "" {
			fmt.Fprintf(buf, "<p class=\"%s\">%s</p>\n", htmlEscape(cls), htmlEscape(content))
		} else {
			fmt.Fprintf(buf, "<p>%s</p>\n", htmlEscape(content))
		}

	case "icon":
		name := comp.GetPropString("name")
		size := comp.GetPropInt("size")
		color := comp.GetPropString("color")
		style := ""
		if size > 0 {
			style = fmt.Sprintf("font-size: %drem;", size)
		}
		cls := name
		if color != "" {
			cls += " text-" + color
		}
		fmt.Fprintf(buf, "<i class=\"%s\" style=\"%s\"></i>\n", htmlEscape(cls), style)

	case "code":
		lang := comp.GetPropString("language")
		content := comp.GetPropString("content")
		cls := comp.GetPropString("class")
		classAttr := ""
		if cls != "" {
			classAttr = fmt.Sprintf(" class=\"%s\"", htmlEscape(cls))
		}
		fmt.Fprintf(buf, "<pre%s><code class=\"language-%s\">%s</code></pre>\n",
			classAttr, htmlEscape(lang), htmlEscape(content))

	case "image":
		src := comp.GetPropString("src")
		alt := comp.GetPropString("alt")
		cls := comp.GetPropString("class")
		classAttr := ""
		if cls != "" {
			classAttr = fmt.Sprintf(" class=\"%s\"", htmlEscape(cls))
		}
		fmt.Fprintf(buf, "<img src=\"%s\" alt=\"%s\"%s>\n", htmlEscape(src), htmlEscape(alt), classAttr)

	case "separator":
		buf.WriteString("<hr>\n")

	case "spacer":
		buf.WriteString("&nbsp;")

	case "html":
		// 原始 HTML 注入
		content := comp.GetPropString("content")
		buf.WriteString(content)
		buf.WriteString("\n")

	case "raw":
		// 原始内容（不转义）
		content := comp.GetPropString("content")
		buf.WriteString(content)

	default:
		// 未知组件：尝试渲染 children
		log.Printf("[警告] 未知 .nc 组件类型: %s", comp.Component)
		r.renderChildren(buf, comp)
	}
}

// renderChildren 渲染子组件列表
func (r *NCRenderer) renderChildren(buf *strings.Builder, parent *NCComponent) {
	for _, child := range parent.Children {
		r.renderComponent(buf, child)
	}
}

// renderNavbar 渲染导航栏
func (r *NCRenderer) renderNavbar(buf *strings.Builder, comp *NCComponent) {
	brand := comp.GetPropString("brand")
	theme := comp.GetPropString("theme")
	bg := comp.GetPropString("bg")

	navClass := "navbar navbar-expand-lg"
	if theme == "dark" {
		navClass += " navbar-dark"
	} else {
		navClass += " navbar-light"
	}
	if bg != "" {
		navClass += " bg-" + bg
	}

	fmt.Fprintf(buf, "<nav class=\"%s\">\n<div class=\"container-fluid\">\n", navClass)
	if brand != "" {
		fmt.Fprintf(buf, "  <a class=\"navbar-brand\" href=\"#\">%s</a>\n", htmlEscape(brand))
	}
	buf.WriteString("  <button class=\"navbar-toggler\" type=\"button\" data-bs-toggle=\"collapse\" data-bs-target=\"#navbarNav\">\n")
	buf.WriteString("    <span class=\"navbar-toggler-icon\"></span>\n  </button>\n")
	buf.WriteString("  <div class=\"collapse navbar-collapse\" id=\"navbarNav\">\n    <ul class=\"navbar-nav\">\n")

	// 渲染导航项
	if items, ok := comp.GetProp("items").([]interface{}); ok {
		for _, itemRaw := range items {
			if item, ok := itemRaw.(map[string]interface{}); ok {
				label, _ := item["label"].(string)
				href, _ := item["href"].(string)
				active, _ := item["active"].(bool)
				activeClass := ""
				if active {
					activeClass = " active"
				}
				fmt.Fprintf(buf, "      <li class=\"nav-item\"><a class=\"nav-link%s\" href=\"%s\">%s</a></li>\n",
					activeClass, htmlEscape(href), htmlEscape(label))
			}
		}
	}

	buf.WriteString("    </ul>\n  </div>\n</div>\n</nav>\n")
}

// renderCard 渲染卡片组件
func (r *NCRenderer) renderCard(buf *strings.Builder, comp *NCComponent) {
	title := comp.GetPropString("title")
	cls := comp.GetPropString("class")
	cardClass := "card"
	if cls != "" {
		cardClass += " " + cls
	}

	fmt.Fprintf(buf, "<div class=\"%s\">\n<div class=\"card-body\">\n", cardClass)
	if title != "" {
		fmt.Fprintf(buf, "  <h5 class=\"card-title\">%s</h5>\n", htmlEscape(title))
	}
	r.renderChildren(buf, comp)
	buf.WriteString("</div>\n</div>\n")
}

// renderButton 渲染按钮
func (r *NCRenderer) renderButton(buf *strings.Builder, comp *NCComponent) {
	text := comp.GetPropString("text")
	color := comp.GetPropString("color")
	outline := comp.GetPropBool("outline")
	disabled := comp.GetPropBool("disabled")
	loading := comp.GetPropBool("loading")
	btnType := comp.GetPropString("type")
	onClick := comp.GetPropString("onClick")
	dismiss := comp.GetPropBool("dismiss")

	if color == "" {
		color = "primary"
	}
	if btnType == "" {
		btnType = "button"
	}

	btnClass := "btn"
	if outline {
		btnClass += " btn-outline-" + color
	} else {
		btnClass += " btn-" + color
	}

	attrs := fmt.Sprintf("type=\"%s\" class=\"%s\"", btnType, btnClass)
	if disabled || loading {
		attrs += " disabled"
	}
	if onClick != "" {
		attrs += fmt.Sprintf(" onclick=\"%s\"", htmlEscape(onClick))
	}
	if dismiss {
		attrs += " data-bs-dismiss=\"modal\""
	}

	displayText := text
	if loading {
		displayText = `<span class="spinner-border spinner-border-sm me-1"></span>` + text
	}

	fmt.Fprintf(buf, "<button %s>%s</button>\n", attrs, displayText)
}

// renderInput 渲染输入框
func (r *NCRenderer) renderInput(buf *strings.Builder, comp *NCComponent) {
	id := comp.GetPropString("id")
	label := comp.GetPropString("label")
	inputType := comp.GetPropString("type")
	placeholder := comp.GetPropString("placeholder")
	required := comp.GetPropBool("required")

	if inputType == "" {
		inputType = "text"
	}

	if label != "" {
		fmt.Fprintf(buf, "<div class=\"mb-3\">\n<label for=\"%s\" class=\"form-label\">%s</label>\n",
			htmlEscape(id), htmlEscape(label))
	}

	requiredAttr := ""
	if required {
		requiredAttr = " required"
	}
	fmt.Fprintf(buf, "<input type=\"%s\" class=\"form-control\" id=\"%s\" placeholder=\"%s\"%s>\n",
		htmlEscape(inputType), htmlEscape(id), htmlEscape(placeholder), requiredAttr)

	if label != "" {
		buf.WriteString("</div>\n")
	}
}

// renderSelect 渲染下拉选择框
func (r *NCRenderer) renderSelect(buf *strings.Builder, comp *NCComponent) {
	id := comp.GetPropString("id")
	label := comp.GetPropString("label")

	if label != "" {
		fmt.Fprintf(buf, "<div class=\"mb-3\">\n<label for=\"%s\" class=\"form-label\">%s</label>\n",
			htmlEscape(id), htmlEscape(label))
	}

	fmt.Fprintf(buf, "<select class=\"form-select\" id=\"%s\">\n", htmlEscape(id))

	if options, ok := comp.GetProp("options").([]interface{}); ok {
		for _, optRaw := range options {
			if opt, ok := optRaw.(map[string]interface{}); ok {
				value, _ := opt["value"].(string)
				optLabel, _ := opt["label"].(string)
				disabled, _ := opt["disabled"].(bool)
				selected, _ := opt["selected"].(bool)
				attrs := fmt.Sprintf("value=\"%s\"", htmlEscape(value))
				if disabled {
					attrs += " disabled"
				}
				if selected {
					attrs += " selected"
				}
				fmt.Fprintf(buf, "  <option %s>%s</option>\n", attrs, htmlEscape(optLabel))
			}
		}
	}

	buf.WriteString("</select>\n")
	if label != "" {
		buf.WriteString("</div>\n")
	}
}

// renderTextarea 渲染文本域
func (r *NCRenderer) renderTextarea(buf *strings.Builder, comp *NCComponent) {
	id := comp.GetPropString("id")
	label := comp.GetPropString("label")
	rows := comp.GetPropInt("rows")
	placeholder := comp.GetPropString("placeholder")

	if rows <= 0 {
		rows = 3
	}

	if label != "" {
		fmt.Fprintf(buf, "<div class=\"mb-3\">\n<label for=\"%s\" class=\"form-label\">%s</label>\n",
			htmlEscape(id), htmlEscape(label))
	}

	fmt.Fprintf(buf, "<textarea class=\"form-control\" id=\"%s\" rows=\"%d\" placeholder=\"%s\"></textarea>\n",
		htmlEscape(id), rows, htmlEscape(placeholder))

	if label != "" {
		buf.WriteString("</div>\n")
	}
}

// renderCheckbox 渲染复选框
func (r *NCRenderer) renderCheckbox(buf *strings.Builder, comp *NCComponent) {
	id := comp.GetPropString("id")
	label := comp.GetPropString("label")
	required := comp.GetPropBool("required")

	requiredAttr := ""
	if required {
		requiredAttr = " required"
	}

	fmt.Fprintf(buf, "<div class=\"mb-3 form-check\">\n"+
		"  <input type=\"checkbox\" class=\"form-check-input\" id=\"%s\"%s>\n"+
		"  <label class=\"form-check-label\" for=\"%s\">%s</label>\n</div>\n",
		htmlEscape(id), requiredAttr, htmlEscape(id), htmlEscape(label))
}

// renderRadioGroup 渲染单选按钮组
func (r *NCRenderer) renderRadioGroup(buf *strings.Builder, comp *NCComponent) {
	id := comp.GetPropString("id")
	label := comp.GetPropString("label")
	inline := comp.GetPropBool("inline")

	if label != "" {
		fmt.Fprintf(buf, "<div class=\"mb-3\">\n<label class=\"form-label\">%s</label>\n", htmlEscape(label))
	}

	if options, ok := comp.GetProp("options").([]interface{}); ok {
		for i, optRaw := range options {
			if opt, ok := optRaw.(map[string]interface{}); ok {
				value, _ := opt["value"].(string)
				optLabel, _ := opt["label"].(string)
				radioID := fmt.Sprintf("%s-%d", id, i)
				checkClass := "form-check"
				if inline {
					checkClass += " form-check-inline"
				}
				fmt.Fprintf(buf, "  <div class=\"%s\">\n"+
					"    <input class=\"form-check-input\" type=\"radio\" name=\"%s\" id=\"%s\" value=\"%s\">\n"+
					"    <label class=\"form-check-label\" for=\"%s\">%s</label>\n  </div>\n",
					checkClass, htmlEscape(id), radioID, htmlEscape(value), radioID, htmlEscape(optLabel))
			}
		}
	}

	if label != "" {
		buf.WriteString("</div>\n")
	}
}

// renderForm 渲染表单
func (r *NCRenderer) renderForm(buf *strings.Builder, comp *NCComponent) {
	id := comp.GetPropString("id")
	onSubmit := comp.GetPropString("onSubmit")

	attrs := ""
	if id != "" {
		attrs += fmt.Sprintf(" id=\"%s\"", htmlEscape(id))
	}
	if onSubmit != "" {
		attrs += fmt.Sprintf(" onsubmit=\"event.preventDefault(); %s\"", htmlEscape(onSubmit))
	}

	fmt.Fprintf(buf, "<form%s>\n", attrs)
	r.renderChildren(buf, comp)
	buf.WriteString("</form>\n")
}

// renderTable 渲染表格
func (r *NCRenderer) renderTable(buf *strings.Builder, comp *NCComponent) {
	striped := comp.GetPropBool("striped")
	hover := comp.GetPropBool("hover")
	bordered := comp.GetPropBool("bordered")
	responsive := comp.GetPropBool("responsive")

	tableClass := "table"
	if striped {
		tableClass += " table-striped"
	}
	if hover {
		tableClass += " table-hover"
	}
	if bordered {
		tableClass += " table-bordered"
	}

	if responsive {
		buf.WriteString("<div class=\"table-responsive\">\n")
	}

	fmt.Fprintf(buf, "<table class=\"%s\">\n<thead>\n<tr>\n", tableClass)

	// 表头
	if headers, ok := comp.GetProp("headers").([]interface{}); ok {
		for _, h := range headers {
			if hs, ok := h.(string); ok {
				fmt.Fprintf(buf, "  <th>%s</th>\n", htmlEscape(hs))
			}
		}
	}
	buf.WriteString("</tr>\n</thead>\n<tbody>\n")

	// 数据行
	if rows, ok := comp.GetProp("rows").([]interface{}); ok {
		for _, rowRaw := range rows {
			if row, ok := rowRaw.(map[string]interface{}); ok {
				buf.WriteString("<tr>\n")
				if cells, ok := row["cells"].([]interface{}); ok {
					for _, cell := range cells {
						if cs, ok := cell.(string); ok {
							fmt.Fprintf(buf, "  <td>%s</td>\n", htmlEscape(cs))
						}
					}
				}
				// 操作列
				if actions, ok := row["actions"].([]interface{}); ok {
					buf.WriteString("  <td>")
					for _, actRaw := range actions {
						if act, ok := actRaw.(map[string]interface{}); ok {
							actLabel, _ := act["label"].(string)
							actColor, _ := act["color"].(string)
							actOnClick, _ := act["onClick"].(string)
							if actColor == "" {
								actColor = "primary"
							}
							fmt.Fprintf(buf, "<button class=\"btn btn-sm btn-%s me-1\" onclick=\"%s\">%s</button>",
								htmlEscape(actColor), htmlEscape(actOnClick), htmlEscape(actLabel))
						}
					}
					buf.WriteString("</td>\n")
				}
				buf.WriteString("</tr>\n")
			}
		}
	}

	buf.WriteString("</tbody>\n</table>\n")
	if responsive {
		buf.WriteString("</div>\n")
	}
}

// renderAlert 渲染提示框
func (r *NCRenderer) renderAlert(buf *strings.Builder, comp *NCComponent) {
	alertType := comp.GetPropString("type")
	text := comp.GetPropString("text")
	dismissible := comp.GetPropBool("dismissible")

	if alertType == "" {
		alertType = "info"
	}

	alertClass := fmt.Sprintf("alert alert-%s", alertType)
	if dismissible {
		alertClass += " alert-dismissible fade show"
	}

	fmt.Fprintf(buf, "<div class=\"%s\" role=\"alert\">\n  %s\n", alertClass, htmlEscape(text))
	if dismissible {
		buf.WriteString("  <button type=\"button\" class=\"btn-close\" data-bs-dismiss=\"alert\"></button>\n")
	}
	buf.WriteString("</div>\n")
}

// renderBadge 渲染徽章
func (r *NCRenderer) renderBadge(buf *strings.Builder, comp *NCComponent) {
	text := comp.GetPropString("text")
	color := comp.GetPropString("color")
	pill := comp.GetPropBool("pill")

	if color == "" {
		color = "primary"
	}

	badgeClass := "badge bg-" + color
	if pill {
		badgeClass += " rounded-pill"
	}

	fmt.Fprintf(buf, "<span class=\"%s\">%s</span>\n", badgeClass, htmlEscape(text))
}

// renderProgress 渲染进度条
func (r *NCRenderer) renderProgress(buf *strings.Builder, comp *NCComponent) {
	value := comp.GetPropInt("value")
	max := comp.GetPropInt("max")
	color := comp.GetPropString("color")
	striped := comp.GetPropBool("striped")
	animated := comp.GetPropBool("animated")
	label := comp.GetPropString("label")

	if max <= 0 {
		max = 100
	}
	percent := float64(value) / float64(max) * 100

	barClass := "progress-bar"
	if color != "" {
		barClass += " bg-" + color
	}
	if striped {
		barClass += " progress-bar-striped"
	}
	if animated {
		barClass += " progress-bar-animated"
	}

	buf.WriteString("<div class=\"progress mb-2\">\n")
	fmt.Fprintf(buf, "  <div class=\"%s\" role=\"progressbar\" style=\"width: %.1f%%\">%s</div>\n",
		barClass, percent, htmlEscape(label))
	buf.WriteString("</div>\n")
}

// renderModal 渲染模态框
func (r *NCRenderer) renderModal(buf *strings.Builder, comp *NCComponent) {
	id := comp.GetPropString("id")
	title := comp.GetPropString("title")
	size := comp.GetPropString("size")

	if id == "" {
		id = "modal-" + fmt.Sprintf("%d", time.Now().UnixNano())
	}

	sizeClass := ""
	if size != "" {
		sizeClass = fmt.Sprintf(" modal-%s", size)
	}

	fmt.Fprintf(buf, "<div class=\"modal fade\" id=\"%s\" tabindex=\"-1\">\n"+
		"<div class=\"modal-dialog%s\">\n<div class=\"modal-content\">\n", id, sizeClass)

	// 头部
	if title != "" {
		fmt.Fprintf(buf, "<div class=\"modal-header\">\n"+
			"  <h5 class=\"modal-title\">%s</h5>\n"+
			"  <button type=\"button\" class=\"btn-close\" data-bs-dismiss=\"modal\"></button>\n"+
			"</div>\n", htmlEscape(title))
	}

	// 内容
	buf.WriteString("<div class=\"modal-body\">\n")
	r.renderChildren(buf, comp)
	buf.WriteString("</div>\n")

	// 底部按钮
	if footer, ok := comp.GetProp("footer").([]interface{}); ok {
		buf.WriteString("<div class=\"modal-footer\">\n")
		for _, btnRaw := range footer {
			btnBytes, _ := json.Marshal(btnRaw)
			btnComp := &NCComponent{}
			btnComp.UnmarshalJSON(btnBytes)
			r.renderComponent(buf, btnComp)
		}
		buf.WriteString("</div>\n")
	}

	buf.WriteString("</div>\n</div>\n</div>\n")
}

// renderTabs 渲染选项卡
func (r *NCRenderer) renderTabs(buf *strings.Builder, comp *NCComponent) {
	id := comp.GetPropString("id")
	if id == "" {
		id = "tabs"
	}

	if tabs, ok := comp.GetProp("tabs").([]interface{}); ok {
		// 标签导航
		fmt.Fprintf(buf, "<ul class=\"nav nav-tabs\" id=\"%s\" role=\"tablist\">\n", id)
		for i, tabRaw := range tabs {
			if tab, ok := tabRaw.(map[string]interface{}); ok {
				tabID, _ := tab["id"].(string)
				tabLabel, _ := tab["label"].(string)
				active, _ := tab["active"].(bool)
				activeClass := ""
				selected := "false"
				if active || i == 0 {
					activeClass = " active"
					selected = "true"
				}
				fmt.Fprintf(buf, "  <li class=\"nav-item\" role=\"presentation\">\n"+
					"    <button class=\"nav-link%s\" id=\"%s-tab\" data-bs-toggle=\"tab\" "+
					"data-bs-target=\"#%s\" type=\"button\" role=\"tab\" aria-selected=\"%s\">%s</button>\n"+
					"  </li>\n", activeClass, tabID, tabID, selected, htmlEscape(tabLabel))
			}
		}
		buf.WriteString("</ul>\n")

		// 标签内容
		fmt.Fprintf(buf, "<div class=\"tab-content\" id=\"%sContent\">\n", id)
		for i, tabRaw := range tabs {
			if tab, ok := tabRaw.(map[string]interface{}); ok {
				tabID, _ := tab["id"].(string)
				active, _ := tab["active"].(bool)
				showActive := ""
				if active || i == 0 {
					showActive = " show active"
				}
				fmt.Fprintf(buf, "  <div class=\"tab-pane fade%s\" id=\"%s\" role=\"tabpanel\">\n", showActive, tabID)

				// 渲染 tab 的 children
				if children, ok := tab["children"].([]interface{}); ok {
					for _, childRaw := range children {
						childBytes, _ := json.Marshal(childRaw)
						childComp := &NCComponent{}
						childComp.UnmarshalJSON(childBytes)
						r.renderComponent(buf, childComp)
					}
				}

				buf.WriteString("  </div>\n")
			}
		}
		buf.WriteString("</div>\n")
	}
}

// renderAccordion 渲染手风琴
func (r *NCRenderer) renderAccordion(buf *strings.Builder, comp *NCComponent) {
	id := comp.GetPropString("id")
	if id == "" {
		id = "accordion"
	}

	fmt.Fprintf(buf, "<div class=\"accordion\" id=\"%s\">\n", id)

	if items, ok := comp.GetProp("items").([]interface{}); ok {
		for i, itemRaw := range items {
			if item, ok := itemRaw.(map[string]interface{}); ok {
				itemID, _ := item["id"].(string)
				itemTitle, _ := item["title"].(string)
				expanded, _ := item["expanded"].(bool)

				if itemID == "" {
					itemID = fmt.Sprintf("%s-item-%d", id, i)
				}

				collapseShow := ""
				collapsedClass := "collapsed"
				ariaExpanded := "false"
				if expanded || i == 0 {
					collapseShow = " show"
					collapsedClass = ""
					ariaExpanded = "true"
				}

				fmt.Fprintf(buf, "  <div class=\"accordion-item\">\n"+
					"    <h2 class=\"accordion-header\">\n"+
					"      <button class=\"accordion-button %s\" type=\"button\" data-bs-toggle=\"collapse\" "+
					"data-bs-target=\"#%s\" aria-expanded=\"%s\">\n        %s\n      </button>\n"+
					"    </h2>\n"+
					"    <div id=\"%s\" class=\"accordion-collapse collapse%s\" data-bs-parent=\"#%s\">\n"+
					"      <div class=\"accordion-body\">\n",
					collapsedClass, itemID, ariaExpanded, htmlEscape(itemTitle),
					itemID, collapseShow, id)

				// 渲染 accordion item 的 children
				if children, ok := item["children"].([]interface{}); ok {
					for _, childRaw := range children {
						childBytes, _ := json.Marshal(childRaw)
						childComp := &NCComponent{}
						childComp.UnmarshalJSON(childBytes)
						r.renderComponent(buf, childComp)
					}
				}

				buf.WriteString("      </div>\n    </div>\n  </div>\n")
			}
		}
	}

	buf.WriteString("</div>\n")
}

// renderList 渲染无序列表
func (r *NCRenderer) renderList(buf *strings.Builder, comp *NCComponent) {
	cls := comp.GetPropString("class")
	classAttr := ""
	if cls != "" {
		classAttr = fmt.Sprintf(" class=\"%s\"", htmlEscape(cls))
	}

	fmt.Fprintf(buf, "<ul%s>\n", classAttr)
	if items, ok := comp.GetProp("items").([]interface{}); ok {
		for _, item := range items {
			if s, ok := item.(string); ok {
				fmt.Fprintf(buf, "  <li>%s</li>\n", htmlEscape(s))
			}
		}
	}
	buf.WriteString("</ul>\n")
}

// renderListGroup 渲染列表组
func (r *NCRenderer) renderListGroup(buf *strings.Builder, comp *NCComponent) {
	buf.WriteString("<ul class=\"list-group\">\n")
	if items, ok := comp.GetProp("items").([]interface{}); ok {
		for _, itemRaw := range items {
			if item, ok := itemRaw.(map[string]interface{}); ok {
				text, _ := item["text"].(string)
				badge, _ := item["badge"].(string)
				if badge != "" {
					fmt.Fprintf(buf, "  <li class=\"list-group-item d-flex justify-content-between align-items-center\">\n"+
						"    %s\n    <span class=\"badge bg-primary rounded-pill\">%s</span>\n  </li>\n",
						htmlEscape(text), htmlEscape(badge))
				} else {
					fmt.Fprintf(buf, "  <li class=\"list-group-item\">%s</li>\n", htmlEscape(text))
				}
			}
		}
	}
	buf.WriteString("</ul>\n")
}

// ============================================================
// 工具函数
// ============================================================

// htmlEscape HTML 特殊字符转义
func htmlEscape(s string) string {
	replacer := strings.NewReplacer(
		"&", "&amp;",
		"<", "&lt;",
		">", "&gt;",
		"\"", "&quot;",
		"'", "&#39;",
	)
	return replacer.Replace(s)
}

// generateAESKey 生成随机 AES-256 密钥
func generateAESKey() ([]byte, error) {
	key := make([]byte, AES_KEY_SIZE)
	_, err := rand.Read(key)
	if err != nil {
		return nil, fmt.Errorf("生成 AES 密钥失败: %w", err)
	}
	return key, nil
}

// encryptData AES-256-GCM 加密
func encryptData(plaintext, key []byte) ([]byte, error) {
	block, err := aes.NewCipher(key)
	if err != nil {
		return nil, fmt.Errorf("创建 AES 密码器失败: %w", err)
	}

	gcm, err := cipher.NewGCM(block)
	if err != nil {
		return nil, fmt.Errorf("创建 GCM 失败: %w", err)
	}

	nonce := make([]byte, gcm.NonceSize())
	if _, err := rand.Read(nonce); err != nil {
		return nil, fmt.Errorf("生成 nonce 失败: %w", err)
	}

	// nonce 前置拼接密文
	ciphertext := gcm.Seal(nonce, nonce, plaintext, nil)
	return ciphertext, nil
}

// decryptData AES-256-GCM 解密
func decryptData(ciphertext, key []byte) ([]byte, error) {
	block, err := aes.NewCipher(key)
	if err != nil {
		return nil, fmt.Errorf("创建 AES 密码器失败: %w", err)
	}

	gcm, err := cipher.NewGCM(block)
	if err != nil {
		return nil, fmt.Errorf("创建 GCM 失败: %w", err)
	}

	nonceSize := gcm.NonceSize()
	if len(ciphertext) < nonceSize {
		return nil, fmt.Errorf("密文长度不足")
	}

	nonce, data := ciphertext[:nonceSize], ciphertext[nonceSize:]
	plaintext, err := gcm.Open(nil, nonce, data, nil)
	if err != nil {
		return nil, fmt.Errorf("解密失败: %w", err)
	}

	return plaintext, nil
}

// sha256Hash 计算 SHA256 哈希
func sha256Hash(data []byte) [HASH_LEN]byte {
	return sha256.Sum256(data)
}

// ============================================================
// 文件收集与压缩
// ============================================================

// collectFiles 递归收集目录下所有文件
func collectFiles(dir string) (map[string][]byte, error) {
	files := make(map[string][]byte)

	err := filepath.Walk(dir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		// 跳过目录和隐藏文件
		if info.IsDir() {
			return nil
		}
		base := filepath.Base(path)
		if strings.HasPrefix(base, ".") {
			return nil
		}

		// 读取文件内容
		data, err := os.ReadFile(path)
		if err != nil {
			return fmt.Errorf("读取文件 %s 失败: %w", path, err)
		}

		// 使用相对路径作为键
		relPath, err := filepath.Rel(dir, path)
		if err != nil {
			return err
		}
		// 统一使用正斜杠
		relPath = filepath.ToSlash(relPath)
		files[relPath] = data

		return nil
	})

	if err != nil {
		return nil, fmt.Errorf("遍历目录失败: %w", err)
	}

	return files, nil
}

// createZipArchive 将文件集合打包为 ZIP 归档
func createZipArchive(files map[string][]byte) ([]byte, error) {
	var buf bytes.Buffer
	writer := zip.NewWriter(&buf)

	for name, data := range files {
		header := &zip.FileHeader{
			Name:   name,
			Method: zip.Deflate,
		}
		header.SetModTime(time.Now())

		w, err := writer.CreateHeader(header)
		if err != nil {
			return nil, fmt.Errorf("创建 ZIP 条目 %s 失败: %w", name, err)
		}

		if _, err := w.Write(data); err != nil {
			return nil, fmt.Errorf("写入 ZIP 条目 %s 失败: %w", name, err)
		}
	}

	if err := writer.Close(); err != nil {
		return nil, fmt.Errorf("关闭 ZIP 写入器失败: %w", err)
	}

	return buf.Bytes(), nil
}

// ============================================================
// 打包格式：[executable][encrypted_zip][8-byte length][32-byte sha256]["NEFUPACK"]
// ============================================================

// PackResult 打包结果
type PackResult struct {
	Data      []byte // 最终二进制数据
	Hash      [32]byte
	DataLen   uint64
}

// buildPackage 构建 Nefu 打包文件
func buildPackage(executable []byte, projectFiles map[string][]byte, encrypt bool) (*PackResult, error) {
	// 1. 创建 ZIP 归档
	zipData, err := createZipArchive(projectFiles)
	if err != nil {
		return nil, fmt.Errorf("创建 ZIP 归档失败: %w", err)
	}
	log.Printf("[信息] ZIP 归档大小: %d 字节", len(zipData))

	// 2. 可选加密
	payload := zipData
	if encrypt {
		key, err := generateAESKey()
		if err != nil {
			return nil, err
		}
		encrypted, err := encryptData(zipData, key)
		if err != nil {
			return nil, fmt.Errorf("加密数据失败: %w", err)
		}
		payload = encrypted
		log.Printf("[信息] 加密后大小: %d 字节", len(payload))
		// 注意：实际产品中密钥需要嵌入可执行文件或通过其他方式传递
		// 这里仅作演示，密钥管理需要更安全的方案
		_ = key
	}

	// 3. 计算哈希
	hash := sha256Hash(payload)

	// 4. 组装二进制格式
	dataLen := uint64(len(payload))
	lenBytes := make([]byte, LEN_SIZE)
	binary.LittleEndian.PutUint64(lenBytes, dataLen)

	var result bytes.Buffer
	result.Write(executable)       // 可执行文件
	result.Write(payload)          // 加密后的 ZIP 数据
	result.Write(lenBytes)         // 8 字节长度
	result.Write(hash[:])          // 32 字节 SHA256
	result.WriteString(NEFUPACK_MAGIC) // 8 字节魔数

	log.Printf("[信息] 打包完成，总大小: %d 字节", result.Len())

	return &PackResult{
		Data:    result.Bytes(),
		Hash:    hash,
		DataLen: dataLen,
	}, nil
}

// readPackage 从打包文件中读取资源数据
func readPackage(exePath string) ([]byte, error) {
	data, err := os.ReadFile(exePath)
	if err != nil {
		return nil, fmt.Errorf("读取打包文件失败: %w", err)
	}

	fileSize := len(data)
	if fileSize < TRAILER_SIZE {
		return nil, fmt.Errorf("文件格式无效：太小")
	}

	// 检查魔数
	magicStart := fileSize - MAGIC_LEN
	magic := string(data[magicStart:])
	if magic != NEFUPACK_MAGIC {
		return nil, fmt.Errorf("文件格式无效：缺少 NEFUPACK 魔数")
	}

	// 读取数据长度
	lenStart := fileSize - TRAILER_SIZE
	dataLen := binary.LittleEndian.Uint64(data[lenStart : lenStart+LEN_SIZE])

	// 读取哈希
	hashStart := lenStart + LEN_SIZE
	storedHash := data[hashStart : hashStart+HASH_LEN]

	// 定位数据起始位置
	dataStart := magicStart - int(dataLen)
	if dataStart < 0 {
		return nil, fmt.Errorf("文件格式无效：数据长度超出范围")
	}

	payload := data[dataStart:magicStart-int(LEN_SIZE+HASH_LEN)]
	// 修正：payload 应该是从 dataStart 到 lenStart
	payload = data[dataStart:lenStart]

	// 验证哈希
	computedHash := sha256Hash(payload)
	if !bytes.Equal(computedHash[:], storedHash) {
		return nil, fmt.Errorf("完整性校验失败：SHA256 不匹配")
	}

	log.Printf("[信息] 成功读取打包数据: %d 字节", len(payload))
	return payload, nil
}

// ============================================================
// 配置加载
// ============================================================

// loadConfig 加载 main.nefu 配置文件
func loadConfig(path string) (*NefuConfig, error) {
	config := &NefuConfig{
		App: AppConfig{
			Name:      "Nefu App",
			Version:   "1.0.0",
			Width:     1024,
			Height:    768,
			Resizable: true,
		},
		Build: BuildConfig{
			Entry:    "index.html",
			Output:   "dist/app.exe",
			Encrypt:  true,
			Compress: true,
		},
		Dev: DevConfig{
			Port:  DEFAULT_PORT,
			Open:  true,
			Watch: true,
		},
		Window: WindowConfig{
			Title:  "Nefu App",
			Center: true,
		},
	}

	if _, err := toml.DecodeFile(path, config); err != nil {
		return nil, fmt.Errorf("解析配置文件 %s 失败: %w", path, err)
	}

	// 同步窗口标题
	if config.Window.Title == "Nefu App" && config.App.Name != "Nefu App" {
		config.Window.Title = config.App.Name
	}

	return config, nil
}

// ============================================================
// 开发服务器
// ============================================================

// DevServer HTTP 开发服务器，支持文件监听和热重载
type DevServer struct {
	config    *NefuConfig
	projectDir string
	server    *http.Server
	watcher   *fsnotify.Watcher
	mu        sync.RWMutex
	ncCache   string // 缓存编译后的 .nc HTML
}

// NewDevServer 创建开发服务器
func NewDevServer(config *NefuConfig, projectDir string) *DevServer {
	return &DevServer{
		config:     config,
		projectDir: projectDir,
	}
}

// Start 启动开发服务器
func (ds *DevServer) Start() error {
	addr := fmt.Sprintf(":%d", ds.config.Dev.Port)

	// 如果入口是 .nc 文件，先编译
	entryPath := filepath.Join(ds.projectDir, ds.config.Build.Entry)
	if strings.HasSuffix(ds.config.Build.Entry, ".nc") {
		if err := ds.compileNC(entryPath); err != nil {
			return fmt.Errorf("编译 .nc 文件失败: %w", err)
		}
	}

	// 创建 HTTP 处理器
	mux := http.NewServeMux()
	mux.HandleFunc("/", ds.handleRequest)

	ds.server = &http.Server{
		Addr:    addr,
		Handler: mux,
	}

	// 启动文件监听
	if ds.config.Dev.Watch {
		go ds.watchFiles()
	}

	log.Printf("[开发] 服务器启动于 http://localhost%s", addr)
	log.Printf("[开发] 入口文件: %s", ds.config.Build.Entry)

	// 自动打开浏览器
	if ds.config.Dev.Open {
		go func() {
			time.Sleep(500 * time.Millisecond)
			openBrowser(fmt.Sprintf("http://localhost:%d", ds.config.Dev.Port))
		}()
	}

	return ds.server.ListenAndServe()
}

// handleRequest 处理 HTTP 请求
func (ds *DevServer) handleRequest(w http.ResponseWriter, r *http.Request) {
	urlPath := r.URL.Path
	if urlPath == "/" {
		urlPath = "/" + ds.config.Build.Entry
	}

	// 如果是 .nc 入口且已编译，返回编译后的 HTML
	if strings.HasSuffix(ds.config.Build.Entry, ".nc") && urlPath == "/"+ds.config.Build.Entry {
		ds.mu.RLock()
		html := ds.ncCache
		ds.mu.RUnlock()

		if html != "" {
			w.Header().Set("Content-Type", "text/html; charset=utf-8")
			w.Write([]byte(html))
			return
		}
	}

	// 静态文件服务
	filePath := filepath.Join(ds.projectDir, filepath.Clean(urlPath))

	// 安全检查：防止路径穿越
	absProject, _ := filepath.Abs(ds.projectDir)
	absFile, _ := filepath.Abs(filePath)
	if !strings.HasPrefix(absFile, absProject) {
		http.Error(w, "禁止访问", http.StatusForbidden)
		return
	}

	info, err := os.Stat(filePath)
	if err != nil || info.IsDir() {
		// 尝试 index.html
		indexPath := filepath.Join(filePath, "index.html")
		if _, err := os.Stat(indexPath); err == nil {
			filePath = indexPath
		} else {
			http.NotFound(w, r)
			return
		}
	}

	http.ServeFile(w, r, filePath)
}

// compileNC 编译 .nc 文件为 HTML
func (ds *DevServer) compileNC(ncPath string) error {
	data, err := os.ReadFile(ncPath)
	if err != nil {
		return fmt.Errorf("读取 .nc 文件失败: %w", err)
	}

	root := &NCComponent{}
	if err := json.Unmarshal(data, root); err != nil {
		return fmt.Errorf("解析 .nc JSON 失败: %w", err)
	}

	renderer := NewNCRenderer()
	html := renderer.Render(root)

	ds.mu.Lock()
	ds.ncCache = html
	ds.mu.Unlock()

	log.Printf("[编译] .nc 文件编译成功: %s", ncPath)
	return nil
}

// watchFiles 监听文件变化并触发重载
func (ds *DevServer) watchFiles() {
	watcher, err := fsnotify.NewWatcher()
	if err != nil {
		log.Printf("[警告] 创建文件监听器失败: %v", err)
		return
	}
	defer watcher.Close()
	ds.watcher = watcher

	// 添加项目目录
	err = watcher.Add(ds.projectDir)
	if err != nil {
		log.Printf("[警告] 添加监听目录失败: %v", err)
		return
	}

	log.Printf("[监听] 正在监听文件变化: %s", ds.projectDir)

	for {
		select {
		case event, ok := <-watcher.Events:
			if !ok {
				return
			}
			if event.Op&(fsnotify.Write|fsnotify.Create) != 0 {
				log.Printf("[变化] 文件已修改: %s", event.Name)

				// 如果是 .nc 文件，重新编译
				if strings.HasSuffix(event.Name, ".nc") {
					if err := ds.compileNC(event.Name); err != nil {
						log.Printf("[错误] 重新编译 .nc 失败: %v", err)
					}
				}
			}
		case err, ok := <-watcher.Errors:
			if !ok {
				return
			}
			log.Printf("[监听错误] %v", err)
		}
	}
}

// Stop 停止开发服务器
func (ds *DevServer) Stop() error {
	if ds.server != nil {
		return ds.server.Close()
	}
	return nil
}

// ============================================================
// JS 桥接脚本生成
// ============================================================

// generateBridgeScript 生成前端桥接 JavaScript
func generateBridgeScript(config *NefuConfig) string {
	return fmt.Sprintf(`
// Nefu Bridge Script — 自动生成，请勿手动修改
(function() {
    'use strict';

    var APP_NAME = %q;
    var APP_VERSION = %q;

    // 桥接对象
    window.nefu = window.nefu || {
        invoke: function(method) {
            var args = Array.prototype.slice.call(arguments, 1);
            if (window.external && window.external.invoke) {
                return window.external.invoke(JSON.stringify({
                    method: method,
                    args: args
                }));
            }
            console.warn('[nefu] invoke 不可用（非 Nefu 运行时）');
            return Promise.resolve(null);
        },
        on: function(event, callback) {
            if (!this._listeners) this._listeners = {};
            if (!this._listeners[event]) this._listeners[event] = [];
            this._listeners[event].push(callback);
        },
        off: function(event, callback) {
            if (!this._listeners || !this._listeners[event]) return;
            this._listeners[event] = this._listeners[event].filter(function(cb) {
                return cb !== callback;
            });
        },
        send: function(channel, data) {
            return this.invoke('__send__', channel, data);
        },
        _emit: function(event, data) {
            if (!this._listeners || !this._listeners[event]) return;
            this._listeners[event].forEach(function(cb) {
                try { cb(data); } catch(e) { console.error(e); }
            });
        },
        version: APP_VERSION,
        appName: APP_NAME
    };

    // lj() 快捷方式
    window.lj = window.lj || function() {
        return window.nefu.invoke.apply(window.nefu, arguments);
    };

    console.log('[nefu] Bridge script loaded — ' + APP_NAME + ' v' + APP_VERSION);
})();
`, config.App.Name, config.App.Version)
}

// ============================================================
// 浏览器打开工具
// ============================================================

// openBrowser 在默认浏览器中打开 URL
func openBrowser(url string) {
	var cmd *exec.Cmd
	switch runtime.GOOS {
	case "windows":
		cmd = exec.Command("cmd", "/c", "start", url)
	case "darwin":
		cmd = exec.Command("open", url)
	default:
		cmd = exec.Command("xdg-open", url)
	}
	if err := cmd.Start(); err != nil {
		log.Printf("[警告] 无法打开浏览器: %v", err)
	}
}

// ============================================================
// CLI 命令处理
// ============================================================

// printUsage 打印帮助信息
func printUsage() {
	fmt.Println(`Nefu — 将网页变成桌面应用

用法:
  nefu <command> [options]

命令:
  init <name>     初始化新项目
  start           启动开发服务器（热重载）
  build           打包为可执行文件

init 选项:
  --template      项目模板 (basic|nc) 默认: basic

start 选项:
  --port <port>   指定端口号 默认: 3900
  --no-open       不自动打开浏览器
  --no-watch      禁用文件监听

build 选项:
  --output <path> 输出文件路径
  --no-encrypt    禁用加密
  --platform      目标平台 (windows|darwin|linux)

示例:
  nefu init my-app
  nefu start --port 8080
  nefu build --output dist/app.exe`)
}

// cmdInit 初始化新项目
func cmdInit(args []string) error {
	if len(args) < 1 {
		return fmt.Errorf("请指定项目名称: nefu init <name>")
	}

	name := args[0]
	template := "basic"

	// 解析参数
	for i := 1; i < len(args); i++ {
		if args[i] == "--template" && i+1 < len(args) {
			template = args[i+1]
			i++
		}
	}

	// 创建项目目录
	projectDir := filepath.Join(".", name)
	if err := os.MkdirAll(projectDir, 0755); err != nil {
		return fmt.Errorf("创建项目目录失败: %w", err)
	}

	// 生成配置文件
	configContent := fmt.Sprintf(`[app]
name = "%s"
version = "1.0.0"
width = 1024
height = 768
resizable = true

[build]
entry = "index.html"
preload = "preload.js"
output = "dist/%s.exe"
encrypt = true
compress = true

[dev]
port = 3900
open = true
watch = true

[window]
title = "%s"
center = true
`, name, name, name)

	if template == "nc" {
		configContent = strings.Replace(configContent, `entry = "index.html"`, `entry = "index.nc"`, 1)
	}

	configPath := filepath.Join(projectDir, CONFIG_FILE)
	if err := os.WriteFile(configPath, []byte(configContent), 0644); err != nil {
		return fmt.Errorf("写入配置文件失败: %w", err)
	}

	// 生成入口文件
	if template == "nc" {
		ncContent := `{
  "component": "page",
  "title": "` + name + `",
  "children": [
    {
      "component": "container",
      "children": [
        {
          "component": "heading",
          "level": 1,
          "text": "欢迎使用 ` + name + `"
        },
        {
          "component": "alert",
          "type": "success",
          "text": "项目已成功创建！编辑 index.nc 开始开发。"
        },
        {
          "component": "card",
          "title": "快速开始",
          "children": [
            {
              "component": "text",
              "content": "运行 nefu start 启动开发服务器，修改文件后自动刷新。"
            }
          ]
        }
      ]
    }
  ]
}`
		ncPath := filepath.Join(projectDir, "index.nc")
		if err := os.WriteFile(ncPath, []byte(ncContent), 0644); err != nil {
			return fmt.Errorf("写入 .nc 文件失败: %w", err)
		}
	} else {
		htmlContent := `<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>` + name + `</title>
    <style>
        body { font-family: system-ui; display: flex; align-items: center; justify-content: center; min-height: 100vh; margin: 0; background: #f0f9ff; }
        .card { background: white; padding: 40px; border-radius: 12px; box-shadow: 0 4px 20px rgba(0,0,0,0.1); text-align: center; }
        h1 { color: #3b82f6; margin-bottom: 10px; }
        p { color: #666; }
    </style>
</head>
<body>
    <div class="card">
        <h1>🚀 ` + name + `</h1>
        <p>Nefu 项目已就绪</p>
        <p>编辑此文件开始开发</p>
    </div>
    <script src="preload.js"></script>
</body>
</html>`
		htmlPath := filepath.Join(projectDir, "index.html")
		if err := os.WriteFile(htmlPath, []byte(htmlContent), 0644); err != nil {
			return fmt.Errorf("写入 HTML 文件失败: %w", err)
		}
	}

	// 生成 preload.js
	preloadContent := `// Nefu 预加载脚本
(function() {
    'use strict';
    // 在此初始化桥接 API 和全局逻辑
    console.log('[preload] 预加载脚本已执行');

    window.addEventListener('DOMContentLoaded', function() {
        if (typeof nefu !== 'undefined') {
            nefu.send('webview.ready', { timestamp: Date.now() });
        }
    });
})();
`
	preloadPath := filepath.Join(projectDir, "preload.js")
	if err := os.WriteFile(preloadPath, []byte(preloadContent), 0644); err != nil {
		return fmt.Errorf("写入 preload.js 失败: %w", err)
	}

	fmt.Printf("✅ 项目 '%s' 创建成功！\n", name)
	fmt.Printf("📁 目录: %s\n", projectDir)
	fmt.Printf("📝 模板: %s\n", template)
	fmt.Println()
	fmt.Println("下一步:")
	fmt.Printf("  cd %s\n", name)
	fmt.Println("  nefu start")

	return nil
}

// cmdStart 启动开发服务器
func cmdStart(args []string) error {
	// 查找配置文件
	configPath := CONFIG_FILE
	if _, err := os.Stat(configPath); os.IsNotExist(err) {
		return fmt.Errorf("未找到配置文件 %s，请先运行 'nefu init'", CONFIG_FILE)
	}

	config, err := loadConfig(configPath)
	if err != nil {
		return err
	}

	// 解析命令行参数覆盖
	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--port":
			if i+1 < len(args) {
				fmt.Sscanf(args[i+1], "%d", &config.Dev.Port)
				i++
			}
		case "--no-open":
			config.Dev.Open = false
		case "--no-watch":
			config.Dev.Watch = false
		}
	}

	projectDir, _ := filepath.Abs(".")
	server := NewDevServer(config, projectDir)

	fmt.Printf("🚀 Nefu 开发服务器\n")
	fmt.Printf("   项目: %s v%s\n", config.App.Name, config.App.Version)
	fmt.Printf("   端口: %d\n", config.Dev.Port)
	fmt.Printf("   入口: %s\n", config.Build.Entry)
	fmt.Printf("   热重载: %v\n", config.Dev.Watch)
	fmt.Println()

	return server.Start()
}

// cmdBuild 打包为可执行文件
func cmdBuild(args []string) error {
	// 查找配置文件
	configPath := CONFIG_FILE
	if _, err := os.Stat(configPath); os.IsNotExist(err) {
		return fmt.Errorf("未找到配置文件 %s，请先运行 'nefu init'", CONFIG_FILE)
	}

	config, err := loadConfig(configPath)
	if err != nil {
		return err
	}

	outputPath := config.Build.Output
	noEncrypt := false

	// 解析命令行参数
	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--output":
			if i+1 < len(args) {
				outputPath = args[i+1]
				i++
			}
		case "--no-encrypt":
			noEncrypt = true
		}
	}

	encrypt := config.Build.Encrypt && !noEncrypt

	fmt.Printf("📦 Nefu 打包\n")
	fmt.Printf("   项目: %s v%s\n", config.App.Name, config.App.Version)
	fmt.Printf("   加密: %v\n", encrypt)
	fmt.Printf("   输出: %s\n", outputPath)
	fmt.Println()

	// 收集项目文件
	projectDir, _ := filepath.Abs(".")
	files, err := collectFiles(projectDir)
	if err != nil {
		return fmt.Errorf("收集项目文件失败: %w", err)
	}
	log.Printf("[信息] 收集到 %d 个文件", len(files))

	// 注入桥接脚本
	bridgeScript := generateBridgeScript(config)
	files["__nefu_bridge__.js"] = []byte(bridgeScript)

	// 读取当前可执行文件作为宿主
	exePath, err := os.Executable()
	if err != nil {
		return fmt.Errorf("获取可执行文件路径失败: %w", err)
	}
	executable, err := os.ReadFile(exePath)
	if err != nil {
		return fmt.Errorf("读取可执行文件失败: %w", err)
	}

	// 构建打包文件
	result, err := buildPackage(executable, files, encrypt)
	if err != nil {
		return fmt.Errorf("打包失败: %w", err)
	}

	// 确保输出目录存在
	outputDir := filepath.Dir(outputPath)
	if outputDir != "" && outputDir != "." {
		if err := os.MkdirAll(outputDir, 0755); err != nil {
			return fmt.Errorf("创建输出目录失败: %w", err)
		}
	}

	// 写入输出文件
	if err := os.WriteFile(outputPath, result.Data, 0755); err != nil {
		return fmt.Errorf("写入输出文件失败: %w", err)
	}

	fileInfo, _ := os.Stat(outputPath)
	sizeMB := float64(fileInfo.Size()) / 1024 / 1024

	fmt.Printf("✅ 打包成功！\n")
	fmt.Printf("   文件: %s\n", outputPath)
	fmt.Printf("   大小: %.2f MB\n", sizeMB)
	fmt.Printf("   哈希: %x\n", result.Hash)

	return nil
}

// ============================================================
// 主入口
// ============================================================

func main() {
	// 设置日志格式
	log.SetFlags(log.Ltime | log.Lmicroseconds)

	args := os.Args[1:]

	if len(args) == 0 {
		printUsage()
		os.Exit(0)
	}

	command := args[0]
	subArgs := args[1:]

	var err error

	switch command {
	case "init":
		err = cmdInit(subArgs)
	case "start":
		err = cmdStart(subArgs)
	case "build":
		err = cmdBuild(subArgs)
	case "help", "--help", "-h":
		printUsage()
	case "version", "--version", "-v":
		fmt.Println("Nefu v1.0.0 (Go)")
	default:
		fmt.Fprintf(os.Stderr, "未知命令: %s\n\n", command)
		printUsage()
		os.Exit(1)
	}

	if err != nil {
		fmt.Fprintf(os.Stderr, "\n❌ 错误: %v\n", err)
		os.Exit(1)
	}
}
