## Cleaning checklist app backend

1) install rust 

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
2) build and run

```bash
cargo run
```


#### Access to openapi json
http://localhost:8080/api-doc/openapi.json

#### Acces to swagger
http://localhost:8080/swagger-ui


#### Reset database
```bash
rm -f cleaner.db && touch cleaner.db
```

## OAuth 2.0 Подготовка

Приложение теперь поддерживает дополнительные поля для:
- Верификации email (`email_verified`, `verification_token`)
- Сброса пароля (`reset_token`, `reset_token_expires`)
- Будущей интеграции OAuth 2.0 провайдеров

### Новые возможности:
1. **Верификация email** - пользователи могут подтверждать свои email адреса
2. **Сброс пароля** - система генерации временных токенов для восстановления доступа
3. **OAuth 2.0 готовность** - структура данных подготовлена для интеграции с провайдерами

### Миграции базы данных
Для применения изменений выполните:
```bash
cargo run
```
Приложение автоматически применит миграцию `2003_alter_users_add_oauth_fields.sql`
