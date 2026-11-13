# CRM: единственный нативный Cloudflare Worker

Общая схема и текущая работа: [ARCHITECTURE.md](../ARCHITECTURE.md) и [Issue #352](https://github.com/iamaman11/crm/issues/352). Состояние исходников не означает, что версия уже опубликована или принята.

- `src/lib.rs` — единственная точка входа и явная RPC policy; `GET /api/doctor` выполняет только read-only диагностику.
- `src/access.rs` — штатная проверка подписанного Cloudflare Access JWT и привязка к tenant.
- `src/connect_sales.rs` и его handler modules — generated Connect traits, typed wire/domain mappings и один router.
- Существующие owner-файлы вызывают оригинальные `modules/crm-*` и выполняют D1 SQL/CAS/receipt/audit. Customer360 — read-only Account composition; Sales–Activities — атомарный linked flow.

`--features native-ci` — изолированный workerd-тест RS256 с тестовым JWKS: только loopback и явный локальный флаг. Релиз собирается с `--features native` и проверяет настоящие Access keys. CI сохраняет production artifact до test-only сборки и восстанавливает его после тестов.

Приёмка выполняется в существующем GitHub CI на code push в dev (Markdown-only не запускает сборку): Buf/strict TS, rustfmt/Clippy/domain tests, Rust WASM/workerd/D1 и реальный Chromium через общий generated client. После переключения обязательны тот же браузерный сценарий через Access, D1/audit и Cloudflare Logs на точной Worker version. Read-only RPC не создают вымышленных audit rows. Doctor не заменяет эти проверки. Доказательства и оставшаяся работа фиксируются в #352.
