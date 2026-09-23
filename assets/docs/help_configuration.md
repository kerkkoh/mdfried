# Configuration

The configuration file is created and stored automatically the first time mdfried is run.

The get the exact location on your OS, run `mdfried --print-config`, this will print the location followed by a sample config.

The format is TOML. The sections are explained here.

## Basic

```toml
font_family = "your-font-name"
```
The font that has been autodetected or selected, can be changed to any font.
Run `mdfried --setup` to go through font setup *if setup is available*, i.e. if headers would be rendered as images.

```toml
stdio_query_timeout_ms = 2000
```
The timeout in milliseconds to wait for a TTY response. It may be necessary to increaso on older or exotic machines, OSs or terminals, such as Windows.

```toml
ignore_text_sizing_protocol = false
```
Suppress probing for the [Text Sizing Protocol](https://sw.kovidgoyal.net/kitty/text-sizing-protocol/) on startup.
When `true`, headers are always rendered as images (using the configured font) instead of using the
terminal's native text scaling. Useful if your terminal falsely reports support for the protocol, or
if you prefer the image-rendered look regardless.

```toml
max_image_height = 30
```
The maximum image height as terminal row count. The width is kept proportional at the aspect ratio, and capped at the viewport width.

```toml
watch_debounce_milliseconds = 100
```
The watch "debounce" milliseconds, only used in watch mode (`-w`).

```toml
enable_mouse_capture = false
```
Enables mouse capture and mouse scroll, but loses the ability to select text normally on some terminals.
However, most terminals allow to select text holding the shift key, in this mode.

```toml
url_transform_command = "readable | html2text"
```
Transform URLs with a shell command before parsing as markdown. Used when opening a URL that does not end in `.md`.

```toml
mermaid = "mmdc -i - -o - -e png"
```
Renders code blocks with the `mermaid` language as diagram images using an external command.
The command reads Mermaid source from stdin and writes an image to stdout.

When omitted, the internal image renderer is used in builds with the `mermaid` feature.
Builds without that feature show the source unless an external renderer is configured.

For terminal text, set a command that reads Mermaid source from stdin and prints UTF-8 text to
stdout. mdfried preserves ANSI colors and whitespace. Use `{width}` in the command to substitute
the available document width in columns; no width option is added automatically.

```toml
# [merman-cli](https://github.com/Latias94/merman), built using merman-ascii
mermaid = { text = "merman-cli render - --format unicode --output - --ascii-max-width {width} --ascii-overflow fallback" }

# [mermaid-ascii](https://github.com/AlexanderGrooff/mermaid-ascii)
# mermaid = { text = "mermaid-ascii --max-width {width}" }

# [beautiful-mermaid-cli](https://github.com/okooo5km/beautiful-mermaid-cli),
# a CLI for [beautiful-mermaid](https://github.com/lukilabs/beautiful-mermaid)
# mermaid = { text = "bm ascii" }

# [termaid](https://github.com/fasouto/termaid)
# mermaid = { text = "termaid --width {width}" }
```

Uncomment one alternative to switch renderer. The `bm ascii` command has no width option. Pan
wide results with left/right arrow keys or `h`/`l`; the status bar shows the visible columns.
Surrounding text stays in place. Diagrams scroll with the document and are rendered again when its
width changes. Text rendering works without image support or the `mermaid` build feature.

The previous `mermaid = { termaid = "..." }` form still loads as a text command, but it no longer
adds `--width` automatically. If the command fails, returns empty output, or takes longer than 30
seconds, mdfried keeps the Mermaid source visible and logs the error.

```toml
osc8_links = true
```
Render OSC8 hyperlink escape sequences over links, making them clickable in supporting terminals.

## Padding

```toml
[padding]
type = "centered"
width = 100
```
Centered with a maximum width of 100 columns.

```toml
[padding]
type = "align-left"
```
Align to the left of the terminal. Also has an optional maximum `width`.

## Theme

```toml
[theme]
blockquote_bar = "▌ "
link_desc_open = ""
link_desc_close = ""
link_url_open = "◖"
link_url_close = "◗"
horizontal_rule_char = "─"
task_checked_mark = "[✓] "
blockquote_colors = [
    "202",
    "203",
    "204",
    "205",
    "206",
    "207",
]
link_bg = "237"
link_fg = "4"
prefix_color = "222"
emphasis_color = "220"
code_bg = "236"
code_fg = "203"
hr_color = "240"
table_border_color = "240"
table_header_color = "255"
header_color = "#FFFFFF"
hide_urls = true
```
The theme, including colors, replacement strings, and some markdown options.
