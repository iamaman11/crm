# Cloudflare CRM

Канонический репозиторий — [iamaman11/crm](https://github.com/iamaman11/crm).
Норма — [ARCHITECTURE.md](ARCHITECTURE.md), архитектурная основа —
[#338](https://github.com/iamaman11/crm/issues/338), текущий план развития —
[#352](https://github.com/iamaman11/crm/issues/352). Завершённый cutover —
[#350](https://github.com/iamaman11/crm/issues/350). Он принят на
`272aba214114f3db6247cad115f86d54312e3030`: [CI + live PASS](https://github.com/iamaman11/crm/actions/runs/43302137364).

```text
Protobuf → Buf → generated Rust/TypeScript
React strict TSX + packages/ui → packages/client/cloudflare
→ binary Connect → Rust Access verification/policy
→ оригинальные Rust owners → owner SQL/CAS/receipts/audit → одна D1
```

Один workers-rs Worker `crm-dev`, один встроенный Connect router, одна существующая D1.
Frontend: `web/src/app.tsx` и feature screens. Wire/domain mappings и D1 adapters:
`rust-party/src/connect_sales.rs`, его handler modules и существующие owner-файлы.
Бизнес-правила принадлежат исходным `modules/crm-*`.
Customer360 читает Account-rooted композицию; Sales–Activities сохраняет связанные изменения атомарно.

Контракты проверяет Buf, Rust — компилятор/Clippy/domain tests, границы TypeScript —
compiler/AST. Скомпилированные router/policy сверяются с descriptors и выполненными
workerd-сценариями. Нет второго ручного каталога покрытия и source-text доказательств тестов.
Количество RPC не является процентом готовности приложения.

PR #351 завершён; новые срезы разрабатываются в кандидатных GitHub ветках/PR → dev по #352, с архитектурой #338. Прямые экспериментальные push в dev не используются. Сборки и тесты выполняет
`.github/workflows/cloudflare-crm-spike.yml`. Зависимости устанавливаются через
`pnpm install --frozen-lockfile`, Rust собирается с `--locked`.
CI проверяет contracts/types, formatting/Clippy/domain, WASM/workerd/D1, затем Chromium.

`rust-party/test/typed-browser.mjs` выполняет реальные UI действия через generated client.
`e2e/live.mjs` запускает те же сценарии через настоящий Access с D1 readback.
Test-only `native-ci` и production `native` явно различаются в evidence.
Live verification связывает HTTP request_id, D1 audit и Cloudflare Logs с точной Worker version.
Отсутствие обязательного evidence означает FAIL; скриншот ошибки не является browser PASS.
Doctor — read-only диагностика доступности D1/runtime, не приёмка приложения.

`migrations/0001_initial.sql` создаёт схему с нуля.
`0002_hard_cutover.sql` один раз удаляет тестовые бизнес-данные и пересоздаёт их схему,
сохраняя `crm_users` и `crm_tenants`. Повторное применение предотвращает стандартный
Wrangler migration ledger. Новые изменения схемы оформляются следующими миграциями.
Миграции и fixtures проверяются в CI; live reset разрешён только после проверки кандидата.

Release использует проверенный artifact без повторной сборки. Сначала проверяет
account/DB/SHA, checksum, D1 Time Travel bookmark и сравнивает канонические SQL
с существующим Wrangler migration ledger. **При отсутствии новых миграций** не
останавливает workers.dev и не повторяет D1 apply. При новых миграциях до изменения
D1 сначала закрывает вход и проверяет остановку. Далее публикует один Worker,
проверяет signed Access и точный SHA через `/api/version`, а также отказ
анонимному запросу к тому же маршруту. Полный live Access/browser/D1/Logs E2E
обязателен в обоих случаях. Если приёмка не пройдена, остаётся строгий
fail-closed (публичный вход выключается); никакой слепой rollback несовместимого
Worker или D1 и автоматического принятия версии нет. D1/Access не удаляются.

Privacy verification/execution, Enrichment provider dispatch, автоматическое объединение
личностей и необоснованные consent grants не подключены и не имитируются.
Защищаются два точных принятых состояния: исторический `272aba214114f3db6247cad115f86d54312e3030` и последняя live-принятая версия API/SQL схемы `50370070d2f9bb147baa8cb712ab4f54d174f7b8` ([CI + live](https://github.com/iamaman11/crm/actions/runs/43342945163)). Buf и git diff проверяют контракты и миграции относительно обоих; предыдущий SHA dev/PR base — только диагностика. После новой принятой **схемы** (нового Protobuf или D1 migration) accepted SHA меняется явно вместе с CI+live evidence, а не автоматически по последнему успешному запуску. Metadata-only release не изменяет принятую базу схемы. Новые миграции добавляются отдельно.
UI/Worker файлы совпали побайтово между PR CI 43298969184 и dev CI 43302137364 одного дерева.
Это подтверждает повторяемость данной пары сборок, не любых будущих окружений.
