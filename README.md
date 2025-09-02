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

#### Exit app
```bash
sudo killall -9 cleaner-api
```
