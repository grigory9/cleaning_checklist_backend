-- Создание таблицы users
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Индекс для поиска по email
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);