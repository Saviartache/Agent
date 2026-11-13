use std::{fmt, io};

#[derive(Debug)]
pub enum AgentError {
    /// Манифест не разобран или противоречит сам себе.
    Manifest(String),
    /// Ресурс набора: отсутствует, не декодируется или выходит за пределы.
    Asset {
        file: String,
        reason: String,
    },
    Io(io::Error),
    /// Отказ Win32: имя вызова и `GetLastError`.
    Platform {
        call: &'static str,
        code: u32,
    },
    /// Поток персонажа уже остановлен.
    Stopped,
    /// Очередь команд заполнена: команда отброшена, вызывающий не ждёт.
    Busy,
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manifest(reason) => write!(f, "манифест персонажа: {reason}"),
            Self::Asset { file, reason } => write!(f, "ресурс {file}: {reason}"),
            Self::Io(error) => write!(f, "ввод-вывод: {error}"),
            Self::Platform { call, code } => write!(f, "{call}: ошибка Windows {code}"),
            Self::Stopped => f.write_str("персонаж остановлен"),
            Self::Busy => f.write_str("очередь команд персонажа заполнена"),
        }
    }
}

impl std::error::Error for AgentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for AgentError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
