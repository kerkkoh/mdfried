use std::sync::Arc;

use mdfrier::MarkdownLink;
use ratatui::{
    layout::Size,
    text::{Line, Text},
};
use ratatui_image::{Resize, picker::Picker, sliced::SlicedProtocol};

use crate::error::Error;

use image::load_from_memory;

/// Render terminal text without requiring a font database or an image protocol.
pub async fn render_text(
    cmd: &str,
    lines: &[Line<'static>],
    width: u16,
) -> Result<Text<'static>, Error> {
    use ansi_to_tui::IntoText as _;
    use std::{process::Stdio, time::Duration};
    use tokio::{io::AsyncWriteExt as _, process::Command, time::timeout};

    let diagram = lines
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    let render = async {
        // Only the configured command is shell code. Mermaid source goes through stdin.
        let command = cmd.replace("{width}", &width.max(1).to_string());
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let mut stdin = child.stdin.take().ok_or_else(|| {
            Error::Io(std::io::Error::other(
                "text renderer stdin pipe unavailable",
            ))
        })?;
        let (written, output) = tokio::join!(
            async move {
                let result = stdin.write_all(diagram.as_bytes()).await;
                drop(stdin);
                result
            },
            child.wait_with_output(),
        );
        let output = output?;
        if !output.status.success() {
            return Err(Error::Mermaid(
                format!(
                    "text renderer exited with {}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim(),
                )
                .into(),
            ));
        }
        written?;
        let rendered =
            String::from_utf8(output.stdout).map_err(|err| Error::Mermaid(err.into()))?;
        if rendered.trim().is_empty() {
            return Err(Error::Mermaid(
                "text renderer returned an empty diagram".into(),
            ));
        }
        Ok(rendered.into_text()?)
    };
    timeout(Duration::from_secs(30), render)
        .await
        .map_err(|_| Error::Mermaid("text renderer timed out after 30 seconds".into()))?
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn text_renderer_passes_source_and_width_and_preserves_layout() {
        let source = vec![Line::from("  A[$(echo untouched)] --> B")];
        let text = render_text(
            r#"sh -c 'cat; printf "\n\033[31m  └──→\033[0m\n%s %s\n" "$1" "$2"' renderer --max-width {width}"#,
            &source,
            42,
        )
        .await
        .unwrap();
        assert_eq!(text.lines[0].to_string(), source[0].to_string());
        assert_eq!(text.lines[1].to_string(), "  └──→");
        assert!(
            text.lines[1]
                .spans
                .iter()
                .any(|span| span.style.fg.is_some())
        );
        assert_eq!(text.lines[2].to_string(), "--max-width 42");

        // Renderers without width options receive no extra flags.
        let text = render_text("cat", &source, 42).await.unwrap();
        assert_eq!(text.lines[0].to_string(), source[0].to_string());
    }

    #[tokio::test]
    async fn text_renderer_reports_failed_and_empty_output() {
        for command in [
            "mdfried-nonexistent-renderer",
            "sh -c 'cat >/dev/null; echo invalid >&2; exit 7'",
            "sh -c 'cat >/dev/null'",
        ] {
            assert!(
                render_text(command, &[Line::from("bad input")], 80)
                    .await
                    .is_err()
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires MDFRIED_TEST_TEXT_RENDERER command on PATH"]
    async fn text_renderer_real_diagrams() {
        let command = std::env::var("MDFRIED_TEST_TEXT_RENDERER").unwrap();
        for source in [
            "flowchart TD\n  Start --> Finish",
            "sequenceDiagram\n  Alice->>Bob: Hello",
            "stateDiagram-v2\n  [*] --> Ready\n  Ready --> [*]",
        ] {
            for width in [40, 80] {
                let lines = source
                    .lines()
                    .map(|line| Line::from(line.to_owned()))
                    .collect::<Vec<_>>();
                let text = render_text(&command, &lines, width).await.unwrap();
                println!("{source}\nwidth={width}\n{text}");
                assert!(text.height() > 2);
                assert!(!text.lines.is_empty());
            }
        }
    }
}

pub async fn render_with_cmd(
    cmd: &str,
    lines: &Vec<Line<'static>>,
    width: u16,
    max_height: u16,
    picker: Arc<Picker>,
) -> Result<(SlicedProtocol, Size, Size, MarkdownLink), Error> {
    use std::io::Write as _;
    use std::process::{Command, Stdio};

    let diagram = lines
        .iter()
        .map(|l| l.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let max_size = Size::new(width, max_height);

    let cmd = cmd.to_owned();
    let (sliced, size) = tokio::task::spawn_blocking(move || {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let Some(stdin) = child.stdin.as_mut() else {
            return Err(Error::Io(std::io::Error::other(
                "mermaid_command pipe error",
            )));
        };
        stdin.write_all(diagram.as_bytes())?;

        let output = child.wait_with_output()?;
        let dyn_img = load_from_memory(&output.stdout)?;
        let size = Resize::Fit(None).size_for(&dyn_img, picker.font_size(), max_size);
        let sliced = SlicedProtocol::new(&picker, dyn_img, Some(size))?;
        Ok::<_, Error>((sliced, size))
    })
    .await??;

    let link = MarkdownLink {
        url: String::new(),
        description: "mermaid".to_owned(),
    };
    Ok((sliced, size, max_size, link))
}

#[cfg(feature = "mermaid")]
pub mod internal {
    use super::*;
    use crate::document::svg_tree_to_rgba;
    use cosmic_text::fontdb::Database;
    use image::DynamicImage;
    use mermaid_rs_renderer::Theme;

    #[cfg(feature = "mermaid")]
    pub async fn render(
        lines: &Vec<Line<'static>>,
        width: u16,
        max_height: u16,
        fontdb: Arc<Database>,
        picker: Arc<Picker>,
    ) -> Result<(SlicedProtocol, Size, Size, MarkdownLink), Error> {
        let diagram = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let max_width_px = width as f32 * picker.font_size().width as f32;
        let max_size = Size::new(width, max_height);

        let (sliced, size) = tokio::task::spawn_blocking(move || {
            let dyn_img = render_image(&diagram, fontdb, max_width_px, None)?;
            let size = Resize::Fit(None).size_for(&dyn_img, picker.font_size(), max_size);
            let sliced = SlicedProtocol::new(&picker, dyn_img, Some(size))?;
            Ok::<_, Error>((sliced, size))
        })
        .await??;

        let link = MarkdownLink {
            url: String::new(),
            description: "mermaid".to_owned(),
        };
        Ok((sliced, size, max_size, link))
    }

    #[cfg(feature = "mermaid")]
    fn render_image(
        diagram: &str,
        fontdb: Arc<Database>,
        max_width_px: f32,
        background: Option<String>,
    ) -> Result<DynamicImage, Error> {
        use mermaid_rs_renderer::{LayoutConfig, compute_layout, parse_mermaid, render_svg};
        use resvg::usvg;

        let parsed = parse_mermaid(diagram).map_err(|err| Error::Mermaid(err.into()))?;

        const DEFAULT_BACKGROUND: &str = "#1E1E1E";
        let theme = dark_mermaid_theme(background.unwrap_or(DEFAULT_BACKGROUND.to_owned()));
        let config = LayoutConfig::default();
        let layout = compute_layout(&parsed.graph, &theme, &config);

        let svg = render_svg(&layout, &theme, &config);

        let options = usvg::Options {
            fontdb,
            ..Default::default()
        };
        let tree = usvg::Tree::from_data(svg.as_bytes(), &options)
            .map_err(|err| Error::Mermaid(err.into()))?;

        let svg_width = tree.size().width();
        if svg_width > max_width_px {
            log::warn!(
                "mermaid diagram too wide ({svg_width:.0}px > {max_width_px:.0}px), skipping render"
            );
            return Err(Error::MermaidTooBig);
        }

        svg_tree_to_rgba(tree)
    }

    #[cfg(feature = "mermaid")]
    fn dark_mermaid_theme(background: String) -> Theme {
        Theme {
            background,
            primary_color: "#2B2D40".to_owned(),
            primary_text_color: "#D4D4D4".to_owned(),
            primary_border_color: "#6B7AA8".to_owned(),
            line_color: "#7A8FA8".to_owned(),
            secondary_color: "#3A3820".to_owned(),
            tertiary_color: "#2B2D40".to_owned(),
            edge_label_background: "rgba(30,30,30,0.92)".to_owned(),
            cluster_background: "#2A2A18".to_owned(),
            cluster_border: "#8A8A30".to_owned(),
            sequence_actor_fill: "#2D2D2D".to_owned(),
            sequence_actor_border: "#888888".to_owned(),
            sequence_actor_line: "#666666".to_owned(),
            sequence_note_fill: "#3A3820".to_owned(),
            sequence_note_border: "#8A8A30".to_owned(),
            sequence_activation_fill: "#2D2D2D".to_owned(),
            sequence_activation_border: "#888888".to_owned(),
            text_color: "#D4D4D4".to_owned(),
            git_commit_label_color: "#D4D4D4".to_owned(),
            git_commit_label_background: "#2B2D40".to_owned(),
            git_tag_label_color: "#D4D4D4".to_owned(),
            git_tag_label_background: "#2B2D40".to_owned(),
            git_tag_label_border: "hsl(240, 40%, 40%)".to_owned(),
            pie_colors: [
                "hsl(240, 40%, 35%)".to_owned(),
                "hsl(60, 50%, 30%)".to_owned(),
                "hsl(280, 40%, 35%)".to_owned(),
                "hsl(180, 40%, 30%)".to_owned(),
                "hsl(20, 50%, 35%)".to_owned(),
                "hsl(150, 40%, 30%)".to_owned(),
                "hsl(320, 40%, 35%)".to_owned(),
                "hsl(200, 40%, 35%)".to_owned(),
                "hsl(0, 50%, 35%)".to_owned(),
                "hsl(100, 40%, 30%)".to_owned(),
                "hsl(40, 50%, 30%)".to_owned(),
                "hsl(260, 40%, 35%)".to_owned(),
            ],
            pie_title_text_color: "#D4D4D4".to_owned(),
            pie_section_text_color: "#D4D4D4".to_owned(),
            pie_legend_text_color: "#D4D4D4".to_owned(),
            pie_stroke_color: "#D4D4D4".to_owned(),
            pie_outer_stroke_color: "#888888".to_owned(),
            ..Theme::mermaid_default()
        }
    }
}
