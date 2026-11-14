use std::f32::consts::FRAC_PI_2;

use super::*;

const RIG: &str = r#"{
    "palette": { "pink": "F29C9A", "white": "FFFFFF", "edge": "B9C0CB" },
    "outlines": { "white": { "color": "edge", "width": 0.7 } },
    "bones": [
        { "name": "body", "at": [0, 0, 12] },
        { "name": "head", "parent": "body", "at": [10, 0, 0] },
        { "name": "ear", "parent": "head", "at": [0, 4, 0], "turn": [90, 0, 0] }
    ],
    "parts": [
        { "bone": "body", "color": "pink", "shape": { "ellipsoid": { "radii": [12, 10, 9] } } },
        { "bone": "head", "color": "white", "rim": "white", "shape": { "ball": { "radius": 8 } } }
    ],
    "sockets": [ { "name": "tip", "bone": "ear", "at": [3, 0, 0] } ]
}"#;

fn near(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-4
}

fn solved(skeleton: &Skeleton, channels: &[Channel], size: f32) -> World {
    let mut world = World::default();
    skeleton.solve(channels, &[], size, &mut world);
    world
}

fn tip(skeleton: &Skeleton, world: &World) -> Vec3 {
    let socket = skeleton.socket("tip").unwrap();
    skeleton.socket_point(world, socket).unwrap()
}

fn error(text: &str) -> String {
    Skeleton::parse(text).unwrap_err().to_string()
}

#[test]
fn rest_pose_stacks_offsets_and_turns() {
    let skeleton = Skeleton::parse(RIG).unwrap();
    let world = solved(&skeleton, &[], 1.0);
    assert!(near(world.joints()[1].origin, Vec3::new(10.0, 0.0, 12.0)));
    // Ухо повёрнуто на 90° по земле: его «вперёд» — вправо от головы
    assert!(near(tip(&skeleton, &world), Vec3::new(10.0, 7.0, 12.0)));
    assert_eq!(skeleton.parts().len(), 2);
    assert_eq!(skeleton.parts()[1].color, 0xFF_FF_FF);
    assert_eq!(skeleton.outlines()[0].color, 0xB9_C0_CB);
    assert_eq!(skeleton.parts()[1].rim, Some(0));
}

#[test]
fn turning_a_bone_carries_its_children_and_sockets() {
    let skeleton = Skeleton::parse(RIG).unwrap();
    let mut channels = [Channel::default(); 3];
    channels[0].turn[0] = FRAC_PI_2;
    let world = solved(&skeleton, &channels, 1.0);
    assert!(near(world.joints()[1].origin, Vec3::new(0.0, 10.0, 12.0)));
    assert!(near(tip(&skeleton, &world), Vec3::new(-7.0, 10.0, 12.0)));
    // Подъём носа головы поднимает сокет, тело не трогает
    channels[0].turn[0] = 0.0;
    channels[2].turn[1] = FRAC_PI_2;
    let world = solved(&skeleton, &channels, 1.0);
    assert!(near(tip(&skeleton, &world), Vec3::new(10.0, 4.0, 15.0)));
}

#[test]
fn size_scales_everything_and_stretch_only_moves_attachments() {
    let skeleton = Skeleton::parse(RIG).unwrap();
    let world = solved(&skeleton, &[], 2.0);
    assert!(near(tip(&skeleton, &world), Vec3::new(20.0, 14.0, 24.0)));
    let mut channels = [Channel::default(); 3];
    channels[0].stretch = [1.5, 1.0, 0.5];
    let world = solved(&skeleton, &channels, 1.0);
    let head = world.joints()[1];
    assert!(near(head.origin, Vec3::new(15.0, 0.0, 12.0)));
    assert_eq!(head.stretch, [1.0; 3]);
    assert!(near(tip(&skeleton, &world), Vec3::new(15.0, 7.0, 12.0)));
}

#[test]
fn foreign_pose_and_unknown_names_give_none() {
    let skeleton = Skeleton::parse(RIG).unwrap();
    assert_eq!(skeleton.bone("head"), Some(1));
    assert_eq!(skeleton.bone("wing"), None);
    assert_eq!(skeleton.socket("hat"), None);
    assert!(skeleton.socket_point(&World::default(), 0).is_none());
}

#[test]
fn broken_files_are_rejected_with_the_reason() {
    let edit = |from: &str, to: &str| {
        assert!(RIG.contains(from), "{from}");
        error(&RIG.replace(from, to))
    };
    assert!(edit(r#""parent": "body""#, r#""parent": "ear""#).contains("должен стоять раньше"));
    assert!(edit(r#""name": "head""#, r#""name": "body""#).contains("повторяется"));
    assert!(edit(r#""color": "pink""#, r#""color": "blue""#).contains("нет цвета «blue»"));
    assert!(edit("F29C9A", "F29C9").contains("RRGGBB"));
    assert!(edit(r#""radius": 8"#, r#""radius": 0"#).contains("часть 1"));
    assert!(edit(r#""bone": "ear""#, r#""bone": "tail""#).contains("нет кости «tail»"));
    assert!(edit(r#""width": 0.7"#, r#""width": 99"#).contains("обводка «white»"));
    assert!(edit(r#""at": [0, 0, 12]"#, r#""at": [0, 0, 1e9]"#).contains("кость «body»"));
    assert!(edit(r#""rim": "white""#, r#""rim": "white", "glow": 1"#).contains("glow"));
    assert!(edit(r#""rim": "white""#, r#""rim": "gold""#).contains("нет обводки «gold»"));
    assert!(error(r#"{ "palette": {}, "bones": [] }"#).contains("костей"));
    assert!(error(&" ".repeat(file::MAX_RIG_BYTES + 1)).contains("1 МиБ"));
}

#[test]
fn ground_bone_keeps_heights_one_to_one_on_screen() {
    let text = RIG.replace(
        r#"{ "name": "body", "at": [0, 0, 12] }"#,
        r#"{ "name": "body", "at": [0, 0, 12], "ground": true }"#,
    );
    let skeleton = Skeleton::parse(&text).unwrap();
    let world = solved(&skeleton, &[], 1.0);
    // Земная кость тянет высоту: на экране 10 вверх — это 10, а не 8,66
    let tall = world.joints()[0].vector(Vec3::new(0.0, 0.0, 10.0));
    assert!((view(tall).y + 10.0).abs() < 1e-4, "{tall:?}");
    let plain = world.joints()[1].vector(Vec3::new(0.0, 0.0, 10.0));
    assert!((view(plain).y + 8.660_254).abs() < 1e-4);
    // Крепление детей — тоже высота земной кости
    let head = world.joints()[1].origin;
    assert!(near(head, Vec3::new(10.0, 0.0, 12.0)), "{head:?}");
}

#[test]
fn controls_chains_and_facing_parts_are_checked() {
    let with = |part: &str| {
        let text = RIG
            .replace(r#""bones": ["#, r#""controls": ["mud"], "bones": ["#)
            .replace(
                r#"{ "bone": "head", "color": "white", "rim": "white", "shape": { "ball": { "radius": 8 } } }"#,
                part,
            );
        Skeleton::parse(&text).map_err(|error| error.to_string())
    };
    let chain = |through: &str| {
        format!(
            r#"{{ "bone": "body", "color": "pink", "shape": {{ "chain": {{ "through": {through}, "radii": [2, 1], "reach": 9 }} }} }}"#
        )
    };
    let skeleton = with(&chain(r#"["head", "ear"]"#)).unwrap();
    let Shape::Chain { through, reach, .. } = &skeleton.parts()[1].shape else {
        panic!("не цепочка");
    };
    assert_eq!((through.as_slice(), *reach), (&[1, 2][..], Some(9.0)));
    assert_eq!(skeleton.control("mud"), Some(0));
    assert!(with(&chain("[]")).unwrap_err().contains("цепочке"));
    assert!(
        with(&chain(r#"["body"]"#))
            .unwrap_err()
            .contains("свою кость")
    );
    assert!(
        with(&chain(r#"["tail"]"#))
            .unwrap_err()
            .contains("нет кости «tail»")
    );
    let ball = |extra: &str| {
        format!(
            r#"{{ "bone": "head", "color": "pink", {extra} "shape": {{ "ball": {{ "radius": 2 }} }} }}"#
        )
    };
    let shown = with(&ball(
        r#""facing": [1, 2, 3], "sink": 0.5, "behind": true, "show": ["mud"],"#,
    ))
    .unwrap();
    assert_eq!(shown.parts()[1].show, [0]);
    assert!(
        with(&ball(r#""sink": 0.5,"#))
            .unwrap_err()
            .contains("facing")
    );
    // Уходить за кость может и обычная часть, и поднятая к зрителю
    let masked = with(&ball(r#""behind": true, "lift": 3,"#)).unwrap();
    assert!(masked.parts()[1].behind && masked.parts()[1].lift == 3.0);
    let sunk = with(&ball(r#""facing": [0, 0, 0], "sink": 2,"#));
    assert!(sunk.unwrap_err().contains("sink"));
    let unknown = with(&ball(r#""hide": ["dust"],"#));
    assert!(unknown.unwrap_err().contains("нет регулятора «dust»"));
}

#[test]
fn controls_drive_bones_and_rock_tilts_them() {
    let text = RIG
        .replace(r#""bones": ["#, r#""controls": ["wings"], "bones": ["#)
        .replace(
            r#"{ "name": "head", "parent": "body", "at": [10, 0, 0] }"#,
            r#"{ "name": "head", "parent": "body", "at": [10, 0, 0], "stretch": [0, 1, 1],
                "drivers": [ { "control": "wings", "turn": [0, 90, 0], "shift": [0, 0, 4], "stretch": [1, 0, 0] } ] }"#,
        );
    let skeleton = Skeleton::parse(&text).unwrap();
    let pose = |wings: f32| {
        let mut world = World::default();
        skeleton.solve(&[], &[wings], 1.0, &mut world);
        (world.joints()[1], tip(&skeleton, &world))
    };
    // Регулятор на нуле: кость в покое и сложена по своей оси
    let (rest, _) = pose(0.0);
    assert_eq!(rest.stretch, [0.0, 1.0, 1.0]);
    assert!(near(rest.origin, Vec3::new(10.0, 0.0, 12.0)));
    // На единице: поднята, раскрыта и задрана носом вверх вместе с потомками
    let (open, tip) = pose(1.0);
    assert_eq!(open.stretch, [1.0; 3]);
    assert!(near(open.origin, Vec3::new(10.0, 0.0, 16.0)));
    assert!(near(tip, Vec3::new(10.0, 7.0, 16.0)), "{tip:?}");
    let (half, _) = pose(0.5);
    assert!((half.frame.forward.z - 0.5f32.sqrt()).abs() < 1e-4);
    // Развалка клонит макушку вправо, а заданные оси ставят кость как есть
    let mut channels = [Channel::default(); 3];
    channels[0].rock = 0.3;
    let rocked = solved(&skeleton, &channels, 1.0).joints()[0];
    assert!(rocked.frame.up.y > 0.25 && rocked.frame.forward.x > 0.99);
    channels[0].frame = Some(Frame::new(1.0, 0.2, 0.0));
    assert_eq!(
        solved(&skeleton, &channels, 1.0).joints()[0].frame,
        Frame::new(1.0, 0.2, 0.0)
    );
    let unknown = text.replace(r#""control": "wings""#, r#""control": "fins""#);
    assert!(error(&unknown).contains("нет регулятора «fins»"));
}

#[test]
fn flat_or_degenerate_shapes_are_rejected() {
    let with = |shape: &str| error(&RIG.replace(r#"{ "ball": { "radius": 8 } }"#, shape));
    assert!(
        with(r#"{ "capsule": { "from": [0,0,0], "to": [1,0,0], "radii": [0, 0] } }"#)
            .contains("капсулы")
    );
    assert!(with(r#"{ "disc": { "at": [0,0,0], "axes": [[1,0,0], [2,0,0]] } }"#).contains("диска"));
    assert!(with(r#"{ "cube": { "side": 1 } }"#).contains("cube"));
}
