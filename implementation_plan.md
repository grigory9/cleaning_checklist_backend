# План реализации расширения модели User для OAuth 2.0

## Шаги для выполнения в режиме Code

### 1. Создание миграции

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

### 2. Обновление модели User

**Файл:** `src/models.rs`

```rust
// Обновленная структура User
#[derive(Debug, Serialize, Deserialize, ToSchema, FromRow, Clone)]
pub struct User {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub email_verified: bool,
    pub verification_token: Option<String>,
    pub reset_token: Option<String>,
    pub reset_token_expires: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Обновленная структура NewUser
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NewUser {
    pub email: String,
    pub password: String,
    pub email_verified: Option<bool>,
}
```

### 3. Обновление SQL-запросов

**Функция User::create:**
```rust
sqlx::query!(
    r#"
    INSERT INTO users (id, email, password_hash, email_verified, created_at, updated_at)
    VALUES (?, ?, ?, ?, ?, ?)
    "#,
    id,
    email,
    password_hash,
    payload.email_verified.unwrap_or(false), // Использовать переданное значение или false
    now,
    now
)
```

**Функция User::find_by_email:**
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

### 4. Обновление RegisterRequest

**Файл:** `src/api/auth.rs`

```rust
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub email_verified: Option<bool>, // Новое опциональное поле
}
```

### 5. Обновление документации

**Файл:** `README.md`

Добавить раздел:
```markdown
## OAuth 2.0 Подготовка

Приложение теперь поддерживает дополнительные поля для:
- Верификации email (`email_verified`, `verification_token`)
- Сброса пароля (`reset_token`, `reset_token_expires`)
- Будущей интеграции OAuth 2.0 провайдеров

### Новые возможности:
1. **Верификация email** - пользователи могут подтверждать свои email адреса
2. **Сброс пароля** - система генерации временных токенов для восстановления доступа
3. **OAuth 2.0 готовность** - структура данных подготовлена для интеграции с провайдерами
```

## Порядок выполнения

1. **Переключиться в режим Code**
2. **Создать файл миграции** с SQL запросами
3. **Обновить модель User** в src/models.rs
4. **Обновить SQL запросы** в функциях работы с пользователями
5. **Обновить структуры запросов** в auth API
6. **Обновить документацию** в README.md
7. **Протестировать изменения** - запустить миграции и проверить работу

## Тестирование

После выполнения изменений необходимо:
1. Запустить приложение для применения миграций
2. Проверить создание нового пользователя с новыми полями
3. Убедиться, что существующие пользователи работают корректно
4. Проверить, что новые поля доступны в API ответах

## Готовность к OAuth 2.0

После выполнения этих изменений система будет готова к реализации:
- Email верификации через отправку токенов
- Механизма сброса пароля
- Интеграции с OAuth 2.0 провайдерами (Google, GitHub, etc.)