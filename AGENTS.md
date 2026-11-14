# ui-agent — памятка агенту

Персонаж рабочего стола Windows (свинка, гусь, слоник, енот или пингвин) и предметы
вокруг него. Прежде чем
что-либо менять, прочитай две заметки в `docs/`:

- [Персонаж рабочего стола](docs/character.md) — архитектура, карта модулей,
  API `Agent`/`AgentHandle`, `Companion`, формат набора, сценки, как добавить предмет,
  ручной чек-лист для Windows.
- [Визуальный стиль рабочего стола](docs/visual-style.md) —
  **обязательна для всего, что рисуется**: плоский цвет, лёгкая мозаика, тень точками,
  перспектива «плоской земли», палитра, числа и чек-лист приёмки.

Добавляешь нового персонажа — иди по [инструкции с чек-листом](docs/new-character.md):
сложение, файл вида, якорные кости и точки крепления, предметы, сценки, тесты.

Изменил поведение, API или вид — обнови эти заметки в том же изменении.

## Коротко

- Только Windows 10/11 x64; никаких абстракций под другие ОС.
- Ядро (`animation`, `assets`, `behavior`, `motion`, `effects`, `render`, `rig`, `sim`)
  не знает о Windows и проверяется тестами на любой машине. Окна, курсор и мониторы —
  в `platform`, поток — в `runtime`; `unsafe` только в `platform`.
- Новый вид рисуется через `render/shadow.rs` (тень) и `render/pixel.rs` (мозаика),
  своих теней и стилей не заводить.
- Виды тела (`RigKind`: `goose`, `pig`, `elephant`, `raccoon`, `penguin`) делят
  решатели `rig/creature`. Вид — это сложение в `rig/creature/build.rs` (как тело движется) и
  внешность в `assets/<вид>/rig.json` (кости, части, сокеты). Рисует всех один рендер —
  `render::Solid` с буфером глубины; кода отрисовки под вид нет, порядок слоёв не задаётся.
  Новый вид — эти два места, строка в `Character::builtin_rig`, набор в `assets/` и свой
  каталог сценок `sim/stunt/<вид>/`. Формат `rig.json` и словарь имён привода — в заметке
  о персонаже; подгонка вида — `preview_rig --rig=<файл>` без пересборки.
- Сценки (`Stunt`): общие — `sim/stunt/common/`, видовые — `sim/stunt/<вид>/` (по каталогу на вид);
  чья сценка — `Stunt::cast`. Одна сценка — один файл; порядок добавления — в заметке.
- Зависимости: `png`, `serde`, `serde_json`, `windows-sys` — новых не добавлять без нужды.
- Окна приложений подключают персонажа только через `Companion` (`src/companion.rs`), не через
  `Agent`.
- Документация крейта — только в `docs/`; ссылок на проекты, которые его подключают, нет.

## Проверки

```bash
cargo fmt --all -- --check
cargo test -p ui-agent
cargo clippy -p ui-agent --all-targets -- -D warnings
cargo clippy -p ui-agent --all-targets --target x86_64-pc-windows-gnu -- -D warnings
cargo run -p ui-agent --example preview_rig -- pig out.png # вид тела кадрами (pig | goose | elephant | raccoon | penguin)
cargo run -p ui-agent --example preview_stunt -- pig oops out.png --ui 3 --crop 140,110  # сценка кадрами
./scripts/build-package.sh                                 # тестовый пакет в dist/ (все персонажи)
cargo test --manifest-path console/Cargo.toml              # консоль пакета (свой воркспейс)
cargo clippy --manifest-path console/Cargo.toml --all-targets --target x86_64-pc-windows-gnu -- -D warnings
```

Тесты `platform`/`agent` требуют рабочего стола Windows: на другой машине они только
собираются (`cargo test -p ui-agent --target x86_64-pc-windows-gnu --no-run`). Не
объявляй их пройденными без запуска на Windows.
