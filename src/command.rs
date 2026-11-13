//! Команды приложения персонажу.

use std::time::Duration;

use crate::math::Vec2;
use crate::rig::RigKind;

/// Режим самостоятельного поведения.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BehaviorMode {
    /// Свободная жизнь: планировщик сам выбирает намерения.
    Wander,
    /// Стоит на месте.
    Stay,
    FollowCursor,
    ChaseCursor,
    Sleep,
}

/// Состояние приложения, которое персонаж отражает.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Normal,
    /// Держатся, пока приложение не сменит.
    Thinking,
    Busy,
    /// Разовые: персонаж отыгрывает реакцию и сам возвращается в `Normal`.
    Success,
    Warning,
    Error,
}

impl Status {
    pub fn is_momentary(self) -> bool {
        matches!(self, Self::Success | Self::Warning | Self::Error)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Info,
    Success,
    Warning,
    Error,
}

/// Карточка, которую персонаж приносит на экран.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
    pub tone: Tone,
    pub title: String,
    pub body: Option<String>,
    /// Время жизни после того, как карточка положена.
    pub ttl: Duration,
}

impl Notification {
    const TTL: Duration = Duration::from_secs(8);

    pub fn new(tone: Tone, title: impl Into<String>) -> Self {
        Self {
            tone,
            title: title.into(),
            body: None,
            ttl: Self::TTL,
        }
    }

    pub fn info(title: impl Into<String>) -> Self {
        Self::new(Tone::Info, title)
    }

    pub fn success(title: impl Into<String>) -> Self {
        Self::new(Tone::Success, title)
    }

    pub fn warning(title: impl Into<String>) -> Self {
        Self::new(Tone::Warning, title)
    }

    pub fn error(title: impl Into<String>) -> Self {
        Self::new(Tone::Error, title)
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// Лежит, пока пользователь не закроет; переполнение вытесняет её последней.
    pub fn sticky(self) -> Self {
        self.ttl(Duration::MAX)
    }

    pub fn is_sticky(&self) -> bool {
        self.ttl == Duration::MAX
    }
}

const COMMON: Option<RigKind> = None;
const GOOSE: Option<RigKind> = Some(RigKind::Goose);
const PIG: Option<RigKind> = Some(RigKind::Pig);
const DACHSHUND: Option<RigKind> = Some(RigKind::Dachshund);
const ELEPHANT: Option<RigKind> = Some(RigKind::Elephant);
const RACCOON: Option<RigKind> = Some(RigKind::Raccoon);
const PENGUIN: Option<RigKind> = Some(RigKind::Penguin);

/// Таблица сценок: вариант, имя, чья, озорная.
macro_rules! stunts {
    ($($(#[$doc:meta])* $variant:ident = $name:literal, $cast:ident $(, $naughty:ident)?;)*) => {
        /// Сценка, которую персонаж разыгрывает сам. Общие доступны всем, видовые — только
        /// своему телу (`cast`); код сценок — `sim/stunt/{common,<вид>}/`.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Stunt {
            $($(#[$doc])* $variant,)*
        }

        impl Stunt {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];

            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)*
                }
            }

            /// Чья сценка: `None` — общая.
            pub fn cast(self) -> Option<RigKind> {
                match self {
                    $(Self::$variant => $cast,)*
                }
            }

            /// Туалетный и театральный юмор: только с `AgentConfig::naughty`.
            pub fn naughty(self) -> bool {
                match self {
                    $(Self::$variant => stunts!(@naughty $($naughty)?),)*
                }
            }
        }
    };
    (@naughty naughty) => { true };
    (@naughty) => { false };
}

stunts! {
    /// Подбегает к курсору, хватает его клювом или рылом и носится с ним; нужен `cursor_grab`.
    CursorHeist = "cursor_heist", COMMON;
    /// Носится кругами и в конце ошалело трясёт головой.
    Zoomies = "zoomies", COMMON;
    /// Приносит записку с шуткой; только когда других карточек на экране нет.
    Note = "note", COMMON;
    /// Гонится за курсором, не хватая; догнал — тормозит юзом.
    Chase = "chase", COMMON;
    /// Тыкает курсор клювом или рылом; курсор не двигается.
    Boop = "boop", COMMON;
    /// Курсор резко пролетел рядом — подпрыгивает и отскакивает.
    Startle = "startle", COMMON;
    /// Набирает воздух и чихает с отдачей.
    Sneeze = "sneeze", COMMON;
    /// Икает: подскок за подскоком.
    Hiccups = "hiccups", COMMON;
    /// Переступает в ритм, вылетают нотки.
    Dance = "dance", COMMON;
    /// Потягивается и зевает.
    Stretch = "stretch", COMMON;
    /// Молча смотрит на пользователя, поворачивая голову за курсором.
    Stare = "stare", COMMON;
    /// Засыпает; курсор рядом будит.
    Nap = "nap", COMMON;
    /// Приносит из-за края подарок и кладёт у курсора.
    Gift = "gift", COMMON;
    /// Пинает мяч, мяч отскакивает от краёв.
    Ball = "ball", COMMON;
    /// Поскальзывается на банановой кожуре.
    Slip = "slip", COMMON;
    /// Спотыкается о карточку приложения; карточку не трогает.
    TripOverCard = "trip_over_card", COMMON;
    /// Пугается своего следа или тени.
    SelfScare = "self_scare", COMMON;
    /// Прячется за краем экрана и выглядывает.
    Peek = "peek", COMMON;
    /// Оступается у края, падает за экран и вылезает обратно.
    EdgeTumble = "edge_tumble", COMMON;
    /// Гоняется за бабочкой.
    Butterfly = "butterfly", COMMON;
    /// Сверху падает ведро — сплющен в блин и пружинит обратно.
    Flatten = "flatten", COMMON;
    /// Театрально «умирает», пока курсор рядом.
    FakeDeath = "fake_death", COMMON, naughty;
    /// Гусь: с разбега влетает в кучу листьев, листья разлетаются.
    LeafPile = "leaf_pile", GOOSE;
    /// Гусь: с разбега врезается в край экрана, летят перья.
    WallSlam = "wall_slam", GOOSE;
    /// Гусь: гогочет кольцами; бывает, отдача роняет его.
    Honk = "honk", GOOSE;
    /// Гусь: шипит на задержавшийся курсор и делает выпад.
    Hiss = "hiss", GOOSE;
    /// Гусь: строевой шаг через экран.
    March = "march", GOOSE;
    /// Гусь: проскальзывает мимо цели и возвращается.
    Skid = "skid", GOOSE;
    /// Гусь: низкий перелёт; бывает, не взлетает или бьётся о верх.
    Flight = "flight", GOOSE;
    /// Гусь: чистит перья, выпадает пёрышко.
    Preen = "preen", GOOSE;
    /// Гусь: клюёт крошки по одной.
    Crumbs = "crumbs", GOOSE;
    /// Гусь: плещется в луже воды и отряхивается.
    Bath = "bath", GOOSE;
    /// Гусь: вьёт гнездо из листьев, в нём яйцо.
    Nest = "nest", GOOSE;
    /// Гусь: тужится и сносит яйцо, из яйца — стрелочка.
    EggSurprise = "egg_surprise", GOOSE, naughty;
    /// Гусь: рисует клювом узор на земле.
    Scribble = "scribble", GOOSE;
    /// Гусь: тянется к курсору, шея завязывается узлом.
    NeckTangle = "neck_tangle", GOOSE;
    /// Гусь: следом проходят гусята.
    Goslings = "goslings", GOOSE;
    /// Свинка: валяется в луже грязи и потом оставляет грязные следы.
    MudBath = "mud_bath", PIG;
    /// Свинка: кучка посреди экрана, с мухами.
    Oops = "oops", PIG, naughty;
    /// Свинка: «пуф» — и подбрасывает.
    FartJump = "fart_jump", PIG, naughty;
    /// Свинка: спотыкается и пашет пятачком.
    Faceplant = "faceplant", PIG;
    /// Свинка: пузом по экрану, как по льду.
    Slide = "slide", PIG;
    /// Свинка: застревает за краем попой наружу.
    Stuck = "stuck", PIG;
    /// Свинка: отряхивает грязь брызгами.
    ShakeOff = "shake_off", PIG;
    /// Свинка: спит в луже, пузыри.
    MudNap = "mud_nap", PIG;
    /// Свинка: прыгает по цепочке луж.
    PuddleHop = "puddle_hop", PIG;
    /// Свинка: роет рылом и находит жёлудь.
    Dig = "dig", PIG;
    /// Свинка: идёт по запаху курсора.
    Sniff = "sniff", PIG;
    /// Свинка: ест яблоко; переела — раздулась.
    Snack = "snack", PIG;
    /// Свинка: толкает рылом жёлудь через экран.
    Bulldozer = "bulldozer", PIG;
    /// Свинка: крутится за хвостиком.
    TailChase = "tail_chase", PIG;
    /// Свинка: ловит спиной монетки.
    PiggyBank = "piggy_bank", PIG;
    /// Свинка: ножками вверх, дрыгает.
    BellyUp = "belly_up", PIG;
    /// Такса: находит тапок, треплет и приносит к курсору.
    Slipper = "slipper", DACHSHUND;
    /// Такса: роет нору у края экрана и ныряет в неё.
    Burrow = "burrow", DACHSHUND;
    /// Такса: длинное тело застревает на развороте.
    UTurn = "u_turn", DACHSHUND;
    /// Такса: тявкает на курсор.
    Yap = "yap", DACHSHUND;
    /// Слоник: трубит, задрав хобот; нотки и волны.
    Trumpet = "trumpet", ELEPHANT;
    /// Слоник: набирает хоботом воду из лужи и поливает себе спину.
    Shower = "shower", ELEPHANT;
    /// Слоник: пугается «мышки»-курсора и привстаёт на задние ноги.
    MouseScare = "mouse_scare", ELEPHANT;
    /// Слоник: спит, положив хобот на землю, и сопит.
    TrunkNap = "trunk_nap", ELEPHANT;
    /// Енот: полощет яблоко передними лапами в луже.
    Rinse = "rinse", RACCOON;
    /// Енот: роется в мусорке и разбрасывает бумажки.
    Rummage = "rummage", RACCOON;
    /// Енот: прячет записку за спину и отдаёт только по щелчку.
    Stash = "stash", RACCOON;
    /// Енот: встаёт на задние лапы и озирается.
    Lookout = "lookout", RACCOON;
    /// Пингвин: разбег и пузом по экрану, за ним полоса.
    Toboggan = "toboggan", PENGUIN;
    /// Пингвин: поскальзывается, машет ластами и падает ничком.
    Wipeout = "wipeout", PENGUIN;
    /// Пингвин: хлопает ластами зрителю.
    Applause = "applause", PENGUIN;
    /// Пингвин: важно шествует через экран, кивая.
    Strut = "strut", PENGUIN;
}

impl Stunt {
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|stunt| stunt.name() == name)
    }

    /// Сценки тела `rig`; у спрайтового набора (`None`) — только общие.
    pub fn roster(rig: Option<RigKind>) -> impl Iterator<Item = Self> {
        Self::ALL
            .iter()
            .copied()
            .filter(move |stunt| stunt.cast().is_none_or(|kind| Some(kind) == rig))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Появиться; `None` — в точке по умолчанию.
    Spawn(Option<Vec2>),
    /// Уйти с экрана; поток продолжает жить до `Shutdown`.
    Despawn,
    SetBehavior(BehaviorMode),
    /// Разовое действие по имени из манифеста.
    Play(String),
    /// Своё состояние: действие из манифеста держится, пока не придёт `Hold(None)`.
    Hold(Option<String>),
    /// Разыграть сценку.
    Stage(Stunt),
    /// Что-нибудь случайное: разовое действие набора или сценка.
    Surprise,
    MoveTo(Vec2),
    SetStatus(Status),
    Notify(Notification),
    Pause(bool),
    SetVisible(bool),
    /// Закрыть окна и завершить поток.
    Shutdown,
}
