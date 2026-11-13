#!/usr/bin/env bash
#
# build-package.sh — собрать пакет с персонажем рабочего стола для Windows x64.
#
#   1) cargo build --release --target x86_64-pc-windows-gnu --manifest-path console/Cargo.toml
#      (живая консоль-меню, свой воркспейс; в бинарнике все персонажи, смена — в меню)
#   2) upx --best --lzma (жмём КОПИЮ в staging, не трогая cargo-output)
#   3) архив dist/ui-agent-win64.zip (ui-agent.exe + README.txt)
#
# Использование:
#   ./scripts/build-package.sh            # со сжатием
#   ./scripts/build-package.sh --no-upx   # без сжатия

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

TARGET="x86_64-pc-windows-gnu"
USE_UPX=1

fail() { printf '  ✗ %s\n' "$*" >&2; exit 1; }
step() { printf '  ▸ %s\n' "$*"; }

for arg in "$@"; do
    case "$arg" in
        --no-upx) USE_UPX=0 ;;
        -h|--help)
            sed -n '3,/^$/{/^$/!p;}' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) fail "неизвестный аргумент: $arg" ;;
    esac
done

STAGE="dist/ui-agent"
ARCHIVE="$STAGE-win64.zip"
EXE="$STAGE/ui-agent.exe"

step "проверяю инструментарий"
command -v cargo >/dev/null || fail "cargo не найден"
command -v zip >/dev/null || fail "zip не найден"
command -v x86_64-w64-mingw32-gcc >/dev/null || fail "нет mingw-w64 линкера (brew install mingw-w64)"
rustup target list --installed 2>/dev/null | grep -qx "$TARGET" \
    || fail "нет rust-target $TARGET → rustup target add $TARGET"
if [ "$USE_UPX" -eq 1 ]; then
    command -v upx >/dev/null || fail "upx не найден (brew install upx), либо запусти с --no-upx"
fi

MANIFEST="console/Cargo.toml"
# Пути машины сборки не должны попасть в строки бинарника
RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$HOME=~ --remap-path-prefix=$PWD=ui-agent"
export RUSTFLAGS

step "cargo build --release --target $TARGET --manifest-path $MANIFEST"
cargo build --release --locked --target "$TARGET" --manifest-path "$MANIFEST"

TARGET_DIR="$(cargo metadata --manifest-path "$MANIFEST" --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
[ -n "$TARGET_DIR" ] || fail "cargo metadata не назвал target_directory"

step "staging $STAGE"
rm -rf "$STAGE" "$ARCHIVE"
mkdir -p "$STAGE"
cp "$TARGET_DIR/$TARGET/release/ui-agent-console.exe" "$EXE"

if [ "$USE_UPX" -eq 1 ]; then
    step "upx --best --lzma $(basename "$EXE")"
    upx --best --lzma "$EXE" >/dev/null
    upx -t "$EXE" >/dev/null || fail "upx: проверка упакованного exe не прошла"
fi

cat > "$STAGE/README.txt" <<TEXT
Персонаж рабочего стола (ui-agent.exe): свинка, гусь или слоник, Windows 10/11 x64

Запуск: двойной щелчок. Персонаж появляется поверх рабочего стола, ходит сам,
замечает курсор, его можно щёлкнуть и перетащить. В окне консоли — живое меню:
слева разделы, в середине пункты, справа журнал событий.

Разделы:
  Персонаж    сменить тело (гусь / свинка / слоник) на лету, включить перехват курсора
  Действия    сыграть одно действие
  Держать     держать действие как своё состояние; «отпустить»
  Сценки      разыграть сценку
  Состояние   обычное, думает, занят, успех, предупреждение, ошибка
  Режим       гуляет, стоит, идёт или гонится за курсором, спит
  Карточки    принести карточку любого тона
  Прогоны     всё подряд, короткий сценарий, что-нибудь случайное, стоп
  Окно        появиться, уйти, пауза, скрыть, в фон, выход

Клавиши: ↑↓ — пункт, ←→ или 1–9 — раздел, Enter — выполнить; мышью — щелчок по
пункту. p — пауза, h — скрыть/показать, r — случайное, s — остановить прогон,
b — в фон, q или Esc — выход. Правая кнопка по персонажу — меню: Пауза / Скрыть / Выход.

В фон: консоль закрывается совсем, персонаж живёт дальше фоновым процессом.
Управление тогда — его меню по правой кнопке: «Выход» завершает процесс,
«Скрыть» возвращает консоль с меню.

Перехват курсора: персонаж ненадолго уводит курсор (по умолчанию выключено;
отпускает по кнопке мыши или Esc).

Ключи запуска:
  --rig goose     начать с гусём (по умолчанию свинка)
  --rig elephant  начать со слоником
  --grab          сразу с перехватом курсора
  --pack <папка>  свой набор спрайтов
  --background    без консоли (так запускает себя пункт «в фон»)
TEXT

step "архив $ARCHIVE"
( cd dist && zip -r -9 "$(basename "$ARCHIVE")" "$(basename "$STAGE")" >/dev/null )

printf '  ✓ %s (%s)\n' "$ARCHIVE" "$(du -h "$ARCHIVE" | cut -f1 | tr -d ' ')"
