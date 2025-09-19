# Техническое задание: Расширение модели User для OAuth 2.0

## Обзор

Расширение существующей модели User для поддержки функциональности OAuth 2.0, верификации email и сброса пароля.

## Текущая архитектура

### Модель User (текущая)
```rust
pub struct User {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### Таблица users (текущая)
- `id TEXT PRIMARY KEY`
- `email TEXT NOT NULL`
- `password_hash TEXT NOT NULL` 
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`

## Требуемые изменения

### 1. Новая модель User

```rust
pub struct User {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub email_verified: bool,           // Флаг верификации email
    pub verification_token: Option<String>, // Токен для верификации
    pub reset_token: Option<String>,    // Токен для сброса пароля
    pub reset_token_expires: Option<DateTime<Utc>>, // Время истечения токена
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### 2. Миграция базы данных

**Файл:** `migrations/2003_alter_users_add_oauth_fields.sql`

```sql
-- Добавление полей для OAuth 2.0 функциональности
ALTER TABLE users ADD COLUMN email_verified BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE users ADD COLUMN verification_token TEXT;
ALTER TABLE users ADD COLUMN reset_token TEXT;
ALTER TABLE users ADD COLUMN reset_token_expires TEXT;

-- Индексы для оптимизации запросов
CREATE INDEX IF NOT EXISTS idx_users_email_verified ON users(email_verified);
CREATE INDEX IF NOT EXISTS idx_users_verification_token ON users(verification_token);
CREATE INDEX IF NOT EXISTS idx_users_reset_token ON users(reset_token);
```

### 3. Обновление SQL запросов

#### Функция User::create
```rust
INSERT INTO users (id, email, password_hash, email_verified, created_at, updated_at)
VALUES (?, ?, ?, ?, ?, ?)
```

#### Функция User::find_by_email  
```rust
SELECT id, email, password_hash, email_verified, verification_token, 
       reset_token, reset_token_expires, created_at, updated_at
FROM users
WHERE email = ?
```

### 4. Обновление структур данных

#### NewUser struct
```rust
pub struct NewUser {
    pub email: String,
    pub password: String,
    pub email_verified: Option<bool>, // Для обратной совместимости
}
```

#### RegisterRequest struct
```rust
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub email_verified: Option<bool>,
}
```

## Бизнес-логика

### Верификация email
- `email_verified = false` по умолчанию для новых пользователей
- `verification_token` генерируется при регистрации
- После подтверждения email: `email_verified = true`, `verification_token = NULL`

### Сброс пароля
- `reset_token` генерируется при запросе сброса
- `reset_token_expires` устанавливается на 1 час
- После сброса: `reset_token = NULL`, `reset_token_expires = NULL`

## Документация

### README.md обновления
```markdown
## OAuth 2.0 Подготовка

Добавлена поддержка:
- ✅ Верификация email адресов
- ✅ Сброс пароля через временные токены
- ✅ Подготовка к OAuth 2.0 интеграции

Новые поля в API ответах:
- `email_verified` - статус верификации
- Возможность optional полей для токенов
```

## Тестирование

### Сценарии тестирования
1. **Создание пользователя** - проверка значений по умолчанию
2. **Регистрация** - проверка работы с новыми полями
3. **Поиск пользователя** - проверка возврата всех полей
4. **Миграция** - применение миграции к существующей БД

### Ожидаемое поведение
- Существующие пользователи получат `email_verified = false`
- Новые пользователи создаются с корректными значениями по умолчанию
- Все SQL запросы работают без ошибок

## Следующие шаги

После реализации этой задачи можно приступать к:
1. Реализации endpoint'ов для верификации email
2. Реализации endpoint'ов для сброса пароля  
3. Интеграции OAuth 2.0 провайдеров (Google, GitHub, etc.)
4. Добавлению системы уведомлений для email верификации