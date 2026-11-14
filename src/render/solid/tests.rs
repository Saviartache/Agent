use super::*;
use crate::rig::Channel;

const SIDE: usize = 80;
const ANCHOR: Vec2 = Vec2::new(40.0, 60.0);
const RED: u32 = 0xFFFF_0000;
const BLUE: u32 = 0xFF00_00FF;
const WHITE: u32 = 0xFFFF_FFFF;
const EDGE: u32 = 0xFF00_FF00;

const BALL: &str = r#"{ "bone": "root", "color": "red", "shape": { "ball": { "at": [0, 0, 20], "radius": 10 } } }"#;

/// Кость `root` на земле и кость `tip` на ней; части — как в `rig.json`.
fn rig(parts: &str) -> Skeleton {
    let text = format!(
        r#"{{
            "palette": {{ "red": "FF0000", "blue": "0000FF", "white": "FFFFFF", "edge": "00FF00" }},
            "outlines": {{ "white": {{ "color": "edge", "width": 2 }}, "other": {{ "color": "blue", "width": 2 }} }},
            "controls": ["show"],
            "bones": [ {{ "name": "root" }}, {{ "name": "tip", "parent": "root", "at": [0, 0, 3] }} ],
            "parts": [ {parts} ]
        }}"#
    );
    Skeleton::parse(&text).unwrap()
}

fn render_with(skeleton: &Skeleton, channels: &[Channel], size: f32, control: f32) -> Vec<u32> {
    let mut world = World::default();
    skeleton.solve(channels, &[], size, &mut world);
    let mut pixels = vec![0u32; SIDE * SIDE];
    let mut canvas = Canvas::new(&mut pixels, SIDE, SIDE as u32, SIDE as u32).unwrap();
    Solid::default().draw(&mut canvas, ANCHOR, skeleton, &world, &[control]);
    pixels
}

fn render(parts: &str) -> Vec<u32> {
    render_with(&rig(parts), &[], 1.0, 1.0)
}

/// Пиксель над точкой на земле: `right` — вправо, `up` — вверх по экрану.
fn at(pixels: &[u32], right: i32, up: i32) -> u32 {
    let (x, y) = (ANCHOR.x as i32 + right, ANCHOR.y as i32 - up);
    pixels[y as usize * SIDE + x as usize]
}

fn count(pixels: &[u32], color: u32) -> usize {
    pixels.iter().filter(|pixel| **pixel == color).count()
}

#[test]
fn nearer_part_covers_the_farther_one_in_any_file_order() {
    let near = r#"{ "bone": "root", "color": "blue", "shape": { "ball": { "at": [0, 12, 20], "radius": 6 } } }"#;
    for parts in [format!("{BALL}, {near}"), format!("{near}, {BALL}")] {
        let pixels = render(&parts);
        // Ближний шар на экране ниже: земля под ним ближе к зрителю
        assert_eq!(at(&pixels, 0, 11), BLUE);
        assert_eq!(at(&pixels, 0, 24), RED);
        assert_eq!(at(&pixels, 0, 40), 0);
    }
}

#[test]
fn ball_is_round_on_screen() {
    let pixels = render(BALL);
    let row = |up: i32| {
        (-20..20)
            .filter(|x| at(&pixels, *x, up) >> 24 > 127)
            .count() as i32
    };
    let column = (0..50).filter(|up| at(&pixels, 0, *up) >> 24 > 127).count() as i32;
    assert!(
        (row(17) - 20).abs() <= 1 && (column - 20).abs() <= 1,
        "{} {column}",
        row(17)
    );
}

#[test]
fn part_sunk_into_another_shows_only_what_sticks_out() {
    let pixels = render(
        r#"{ "bone": "root", "color": "red", "shape": { "ellipsoid": { "at": [0, 0, 24], "radii": [14, 10, 9] } } },
           { "bone": "root", "color": "blue", "shape": { "capsule": { "from": [0, 0, 24], "to": [0, 0, 3], "radii": [3, 3] } } }"#,
    );
    // Нога из-под брюха: внутри тела её не видно, ниже — видно
    assert_eq!(at(&pixels, 0, 21), RED);
    assert_eq!(at(&pixels, 0, 5), BLUE);
    assert!(count(&pixels, BLUE) < 200);
    assert!(count(&pixels, RED) > 300);
}

#[test]
fn disc_shows_only_facing_the_viewer_and_in_front_of_the_ball() {
    // Диск у шара со стороны `y`; его лицо `axes[0] × axes[1]` смотрит по `−across`
    let eye = |y: f32, across: f32| {
        let parts = format!(
            r#"{BALL}, {{ "bone": "root", "color": "blue", "shape": {{ "disc": {{ "at": [0, {y}, 20], "axes": [[{across}, 0, 0], [0, 0, 3]] }} }} }}"#
        );
        count(&render(&parts), BLUE)
    };
    assert!(eye(10.2, -3.0) > 3);
    // Изнанкой к зрителю
    assert_eq!(eye(10.2, 3.0), 0);
    // На затылке: лицом к зрителю, но за шаром
    assert_eq!(eye(-10.2, -3.0), 0);
    assert_eq!(eye(-10.2, 3.0), 0);
}

#[test]
fn rim_edges_white_outside_it_and_leaves_no_seam_between_rimmed_parts() {
    let white = |at: &str, rim: bool| {
        let rim = if rim { r#""rim": "white","# } else { "" };
        format!(
            r#"{{ "bone": "root", "color": "white", {rim} "shape": {{ "ball": {{ "at": {at}, "radius": 8 }} }} }}"#
        )
    };
    let plain = render(&white("[0, 0, 20]", false));
    let rimmed = render(&white("[0, 0, 20]", true));
    assert_eq!(count(&plain, EDGE), 0);
    assert!(count(&rimmed, EDGE) > 40);
    // Белого не убавилось: кайма снаружи
    assert_eq!(count(&rimmed, WHITE), count(&plain, WHITE));
    // Два белых шара внахлёст, один ближе: между ними каймы нет, только вокруг
    let pair = render(&format!(
        "{}, {}",
        white("[-5, 0, 20]", true),
        white("[5, 6, 20]", true)
    ));
    let inner: Vec<u32> = (-4..=4).map(|x| at(&pair, x, 15)).collect();
    assert!(inner.iter().all(|pixel| *pixel == WHITE), "{inner:x?}");
    // Кайма ложится и на то, что дальше, а не только на пустое
    let over = render(&format!(
        r#"{BALL}, {{ "bone": "root", "color": "white", "rim": "white", "shape": {{ "ball": {{ "at": [0, 10, 20], "radius": 3 }} }} }}"#
    ));
    assert_eq!(at(&over, 4, 12), EDGE);
    assert_eq!(at(&over, 0, 12), WHITE);
    assert_eq!(at(&over, 8, 14), RED);
}

#[test]
fn facing_part_keeps_its_shape_and_hides_behind_a_turned_bone() {
    // Зрачок перед шаром: привязка на «лице» кости, форма — в осях экрана
    let eye = r#"{ "bone": "root", "color": "blue", "facing": [10, 0, 20], "behind": true, "sink": 0.5,
        "shape": { "capsule": { "from": [-3, 0, 2], "to": [3, 0, 2], "radii": [1, 1] } } }"#;
    let skeleton = rig(&format!("{BALL}, {eye}"));
    let turned = |yaw: f32| {
        let mut channel = Channel::default();
        channel.turn[0] = yaw;
        render_with(&skeleton, &[channel], 1.0, 1.0)
    };
    // «Вперёд» кости — к зрителю: черта горизонтальна, в какую сторону ни качни
    for yaw in [1.2, 1.57, 1.9] {
        let pixels = turned(yaw);
        let blue = count(&pixels, BLUE);
        // Сплошь синих немного: черта тонкая, края сглажены
        assert!((4..=20).contains(&blue), "{yaw} {blue}");
    }
    // Отвернулась: привязка на затылке, часть за шаром
    assert_eq!(count(&turned(-1.57), BLUE), 0);
}

#[test]
fn chain_follows_its_bones_stops_at_reach_and_tucks_in() {
    let chain = |reach: &str| {
        rig(&format!(
            r#"{{ "bone": "root", "color": "blue", "shape": {{ "chain": {{ "from": [0, 0, 30], "through": ["tip"], "radii": [2, 2]{reach} }} }} }}"#
        ))
    };
    let tip = |size: f32| {
        let mut channels = [Channel::default(); 2];
        channels[1].size = size;
        channels
    };
    let full = render_with(&chain(""), &tip(1.0), 1.0, 1.0);
    assert_eq!(at(&full, 0, 20), BLUE);
    assert_eq!(at(&full, 0, 4), BLUE);
    let short = render_with(&chain(r#", "reach": 10"#), &tip(1.0), 1.0, 1.0);
    assert_eq!(at(&short, 0, 20), BLUE);
    assert_eq!(at(&short, 0, 10), 0);
    // Наполовину поджатая короче, но не тоньше; поджатой целиком нет
    let half = render_with(&chain(""), &tip(0.5), 1.0, 1.0);
    assert_eq!(at(&half, 0, 20), BLUE);
    assert_eq!(at(&half, 0, 8), 0);
    assert_eq!(
        count(&render_with(&chain(""), &tip(0.0), 1.0, 1.0), BLUE),
        0
    );
}

#[test]
fn controls_show_hide_and_tint() {
    let spot = |switch: &str| {
        rig(&format!(
            r#"{BALL}, {{ "bone": "root", "color": "blue", {switch}, "shape": {{ "ball": {{ "at": [0, 9, 20], "radius": 4 }} }} }}"#
        ))
    };
    let shown = spot(r#""show": ["show"]"#);
    assert_eq!(count(&render_with(&shown, &[], 1.0, 0.0), BLUE), 0);
    assert!(count(&render_with(&shown, &[], 1.0, 1.0), BLUE) > 20);
    let hidden = spot(r#""hide": ["show"]"#);
    assert_eq!(count(&render_with(&hidden, &[], 1.0, 1.0), BLUE), 0);
    // Наполовину: красит то, что под ним, и не рисует на пустом
    let half = render_with(&shown, &[], 1.0, 0.5);
    let tinted = at(&half, 0, 13);
    assert!(
        tinted != RED && tinted != BLUE && tinted >> 24 == 0xFF,
        "{tinted:x}"
    );
    assert_eq!(count(&half, BLUE), 0);
}

#[test]
fn edges_are_smoothed_and_size_scales_the_picture() {
    let skeleton = rig(
        r#"{ "bone": "root", "color": "red", "shape": { "ball": { "at": [0, 0, 12], "radius": 6 } } }"#,
    );
    let small = render_with(&skeleton, &[], 1.0, 1.0);
    let large = render_with(&skeleton, &[], 2.0, 1.0);
    let covered = |pixels: &[u32]| pixels.iter().filter(|p| **p >> 24 != 0).count() as f32;
    let partial = small
        .iter()
        .filter(|p| !matches!(**p >> 24, 0 | 255))
        .count();
    assert!(partial > 8);
    let ratio = covered(&large) / covered(&small);
    assert!((3.4..4.6).contains(&ratio), "{ratio}");
}

#[test]
fn draw_reports_what_it_touched_and_cleans_up_between_frames() {
    let skeleton = rig(BALL);
    let mut world = World::default();
    let mut solid = Solid::default();
    let mut frame = |solid: &mut Solid, shift: f32| {
        let mut channel = Channel::default();
        channel.shift.x = shift;
        skeleton.solve(&[channel], &[], 1.0, &mut world);
        let mut pixels = vec![0u32; SIDE * SIDE];
        let mut canvas = Canvas::new(&mut pixels, SIDE, SIDE as u32, SIDE as u32).unwrap();
        let drawn = solid.draw(&mut canvas, ANCHOR, &skeleton, &world, &[]);
        (pixels, drawn)
    };
    let (first, drawn) = frame(&mut solid, -20.0);
    assert_eq!(drawn[0], 10..30);
    assert!(drawn[1].start >= 32 && drawn[1].end <= 54, "{drawn:?}");
    let outside = first.iter().enumerate().filter(|(i, pixel)| {
        **pixel != 0 && !(drawn[0].contains(&(i % SIDE)) && drawn[1].contains(&(i / SIDE)))
    });
    assert_eq!(outside.count(), 0);
    // Следующий кадр в другом месте: от прошлого ничего не осталось
    let (second, _) = frame(&mut solid, 20.0);
    assert_eq!(at(&second, -20, 17), 0);
    assert_eq!(at(&second, 20, 17), RED);
}

#[test]
fn broken_and_foreign_poses_stay_quiet() {
    let skeleton = rig(BALL);
    let mut channel = Channel::default();
    channel.shift.x = f32::NAN;
    assert_eq!(count(&render_with(&skeleton, &[channel], 1.0, 1.0), RED), 0);
    render_with(&skeleton, &[], f32::INFINITY, f32::NAN);
    let mut pixels = vec![0u32; SIDE * SIDE];
    let mut canvas = Canvas::new(&mut pixels, SIDE, SIDE as u32, SIDE as u32).unwrap();
    Solid::default().draw(&mut canvas, ANCHOR, &skeleton, &World::default(), &[]);
    assert!(pixels.iter().all(|pixel| *pixel == 0));
}
