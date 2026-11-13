//! Кадр консоли: шапка, разделы, пункты, журнал, прогон, подсказки.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, LineGauge, List, Paragraph},
};
use ui_agent::rig::RigKind;

use crate::app::{App, Live, Note};
use crate::menu::{Item, Order, Step};

const DIM: Color = Color::DarkGray;
const ERROR: Color = Color::LightRed;

/// Цвет персонажа: розовый у свинки, клювно-оранжевый у гуся, серый у енота.
fn accent(live: &Live) -> Color {
    match live.rig {
        Some(RigKind::Pig) => Color::Rgb(242, 140, 170),
        Some(RigKind::Goose) => Color::Rgb(250, 176, 60),
        Some(RigKind::Raccoon) => Color::Rgb(168, 159, 148),
        None => Color::Cyan,
    }
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let accent = accent(&app.live);
    let gauge = Constraint::Length(u16::from(app.progress.is_some()));
    let [header, body, progress, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        gauge,
        Constraint::Length(1),
    ])
    .areas(frame.area());
    let [sidebar, list, log] = Layout::horizontal([
        Constraint::Length(16),
        Constraint::Fill(1),
        Constraint::Percentage(40),
    ])
    .areas(body);

    frame.render_widget(head(&app.live, accent), header);
    draw_sidebar(frame, app, sidebar, accent);
    draw_list(frame, app, list, accent);
    draw_log(frame, app, log);
    if let Some((done, total)) = app.progress {
        let gauge = LineGauge::default()
            .ratio(done as f64 / total.max(1) as f64)
            .label(format!(" прогон {done}/{total} · s — стоп "))
            .filled_symbol("━")
            .unfilled_symbol("─")
            .filled_style(Style::new().fg(accent))
            .unfilled_style(Style::new().fg(DIM));
        frame.render_widget(gauge, progress);
    }
    frame.render_widget(hints(accent), footer);
}

fn panel(title: &str, accent: Color) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(DIM))
        .title(Span::styled(
            format!(" {title} "),
            Style::new().fg(accent).bold(),
        ))
}

fn head(live: &Live, accent: Color) -> Line<'static> {
    let chip = |on: bool, yes: &'static str, no: &'static str| {
        let (text, color) = if on { (yes, accent) } else { (no, DIM) };
        Span::styled(format!("  {text}"), Style::new().fg(color))
    };
    Line::from(vec![
        Span::styled(
            " ui-agent ",
            Style::new().fg(Color::Black).bg(accent).bold(),
        ),
        Span::styled(format!(" {} ", live.name), Style::new().fg(accent).bold()),
        chip(!live.paused, "живёт", "пауза"),
        chip(live.visible, "на виду", "скрыт"),
        chip(live.grab, "перехват вкл", "перехват выкл"),
    ])
}

fn draw_sidebar(frame: &mut Frame, app: &mut App, area: Rect, accent: Color) {
    let block = panel("Разделы", accent);
    app.areas.sidebar = block.inner(area);
    let titles = app
        .sections
        .iter()
        .enumerate()
        .map(|(i, section)| Line::from(format!("{} {}", i + 1, section.title)));
    let list = List::new(titles)
        .block(block)
        .highlight_style(Style::new().fg(accent).add_modifier(Modifier::BOLD))
        .highlight_symbol("▌");
    frame.render_stateful_widget(list, area, &mut app.section);
}

fn draw_list(frame: &mut Frame, app: &mut App, area: Rect, accent: Color) {
    let current = app.current();
    let section = &app.sections[current];
    let block = panel(section.title, accent);
    app.areas.list = block.inner(area);
    let rows = section.entries.iter().map(|entry| {
        let mark = mark(&entry.item, &app.live);
        let sign = match mark {
            Some(true) => Span::styled("● ", Style::new().fg(accent)),
            Some(false) => Span::styled("○ ", Style::new().fg(DIM)),
            None => Span::raw("  "),
        };
        Line::from(vec![sign, Span::raw(entry.label.as_str())])
    });
    let list = List::new(rows)
        .block(block)
        .highlight_style(Style::new().fg(Color::Black).bg(accent))
        .highlight_symbol("▌");
    frame.render_stateful_widget(list, area, &mut app.lists[current]);
}

/// Отметка пункта: `Some` — выбор или переключатель, `None` — разовая команда.
fn mark(item: &Item, live: &Live) -> Option<bool> {
    match item {
        Item::Rig(kind) => Some(live.rig == Some(*kind)),
        Item::Grab => Some(live.grab),
        Item::Order(Order::Pause) => Some(live.paused),
        Item::Order(Order::Visible) => Some(!live.visible),
        Item::Order(Order::Step(Step::Mode(mode))) => Some(live.mode == *mode),
        Item::Order(Order::Step(Step::Hold(held))) => Some(live.held == *held),
        _ => None,
    }
}

fn draw_log(frame: &mut Frame, app: &App, area: Rect) {
    let block = panel("Журнал", DIM);
    let height = block.inner(area).height as usize;
    let lines: Vec<_> = app
        .log
        .iter()
        .skip(app.log.len().saturating_sub(height))
        .map(|(kind, text)| match kind {
            Note::Command => Line::from(vec![
                Span::styled("▸ ", Style::new().fg(accent(&app.live))),
                Span::raw(text.as_str()),
            ]),
            Note::Event => Line::styled(format!("  {text}"), Style::new().fg(DIM)),
            Note::Error => Line::styled(format!("✗ {text}"), Style::new().fg(ERROR)),
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn hints(accent: Color) -> Line<'static> {
    const KEYS: [(&str, &str); 9] = [
        ("↑↓", "выбор"),
        ("←→", "раздел"),
        ("Enter", "выполнить"),
        ("p", "пауза"),
        ("h", "скрыть"),
        ("r", "случайное"),
        ("s", "стоп"),
        ("b", "в фон"),
        ("q", "выход"),
    ];
    KEYS.iter()
        .flat_map(|(key, what)| {
            [
                Span::styled(format!(" {key}"), Style::new().fg(accent).bold()),
                Span::styled(format!(" {what} "), Style::new().fg(DIM)),
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};
    use ui_agent::Character;

    use super::*;

    fn screen(app: &mut App) -> String {
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        let buffer = terminal.backend().buffer();
        buffer
            .content()
            .chunks(buffer.area.width as usize)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn frame_shows_rig_sections_and_marks() {
        let mut app = App::new(&Character::builtin(RigKind::Pig).unwrap(), false);
        app.progress = Some((3, 10));
        app.note(Note::Error, "очередь полна".into());
        let screen = screen(&mut app);
        let expected = [
            "свинка",
            "1 Персонаж",
            "9 Окно",
            "● свинка",
            "○ гусь",
            "прогон 3/10",
            "▸ на столе: свинка",
            "✗ очередь полна",
        ];
        for text in expected {
            assert!(screen.contains(text), "нет «{text}»");
        }
        assert_eq!(app.areas.list.height, 24 - 1 - 1 - 1 - 2);
    }
}
