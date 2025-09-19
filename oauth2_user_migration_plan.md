# План миграции для расширения модели User под OAuth 2.0

## Текущее состояние

**Модель User:**
```rust
pub struct User {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

**Таблица users:** Создается автоматически SQLite при первом INSERT запросе

## Требуемые изменения

### 1. Новая структура модели User

```rust
pub struct User {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub email_verified: bool,           // Новое поле: флаг верификации email
    pub verification_token: Option<String>, // Новое поле: токен для верификации
    pub reset_token: Option<String>,    // Новое поле: токен для сброса пароля
    pub reset_token_expires: Option<DateTime<Utc>>, // Новое поле: время истечения токена
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### 2. Миграция для добавления полей

**Файл:** `migrations/2003_alter_users_add_oauth_fields.sql`

```sql
-- Добавление полей для OAuth 2.0 и верификации email
ALTER TABLE users ADD COLUMN email_verified BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE users ADD COLUMN verification_token TEXT;
ALTER TABLE users ADD COLUMN reset_token TEXT;
ALTER TABLE users ADD COLUMN reset_token_expires TEXT;

-- Создание индексов для оптимизации запросов
CREATE INDEX IF NOT EXISTS idx_users_email_verified ON users(email_verified);
CREATE INDEX IF NOT EXISTS idx_users_verification_token ON users(verification_token);
CREATE INDEX IF NOT EXISTS idx_users_reset_token ON users(reset_token);
```

### 3. Обновление SQL-запросов

**Функция `User::create` (src/models.rs):**
```rust
sqlx::query!(
    r#"
    INSERT INTO users (id, email, password_hash, email_verified, created_at, updated_at)
    VALUES (?, ?, ?, ?, ?, ?)
    "#,
    id,
    email,
    password_hash,
    false,  // email_verified по умолчанию false
    now,
    now
)
```

**Функция `User::find_by_email` (src/models.rs):**
```rust
sqlx::query_as!(
    User,
    r#"
    SELECT id, email, password_hash, email_verified, verification_token, 
           reset_token, reset_token_expires, created_at, updated_at
    FROM users
    WHERE email = ?
    "#,
    email
)
```

### 4. Обновление структуры NewUser

```rust
pub struct NewUser {
    pub email: String,
    pub password: String,
    pub email_verified: Option<bool>, // Опционально для обратной совместимости
}
```

### 5. Обновление документации в README.md

Добавить раздел:
```markdown
## OAuth 2.0 и верификация email

Приложение поддерживает:
- Верификацию email через токены
- Сброс пароля с временными токенами
- Подготовку к интеграции OAuth 2.0 провайдеров

Новые поля в таблице users:
- `email_verified` - флаг подтверждения email
- `verification_token` - токен для верификации
- `reset_token` - токен для сброса пароля
- `reset_token_expires` - время истечения токена
```

## Порядок выполнения

1. **Создать миграцию** - файл `migrations/2003_alter_users_add_oauth_fields.sql`
2. **Обновить модель User** - добавить новые поля в struct
3. **Обновить SQL запросы** - в функциях create и find_by_email
4. **Обновить NewUser struct** - добавить опциональное поле
5. **Обновить документацию** - добавить раздел в README.md
6. **Протестировать** - убедиться в корректной работе

## Ожидаемый результат

После выполнения миграции система будет готова к:
- Реализации верификации email
- Реализации сброса пароля
- Интеграции OAuth 2.0 провайдеров (Google, GitHub и др.)