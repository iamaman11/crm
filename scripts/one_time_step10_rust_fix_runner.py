from __future__ import annotations

from pathlib import Path

path = Path("scripts/one_time_step10_rust_fix.py")
text = path.read_text(encoding="utf-8")
insert_call = '''replace_exact(
    source,
    '    .bind(transaction_id(invocation))\\n    .execute(&mut **transaction)\\n',
    '    .bind(business_transaction_id)\\n    .execute(&mut **transaction)\\n',
    "insert reference business transaction binding",
)
'''
update_call = '''replace_exact(
    source,
    '    .bind(transaction_id(invocation))\\n    .execute(&mut **transaction)\\n',
    '    .bind(business_transaction_id)\\n    .execute(&mut **transaction)\\n',
    "update reference business transaction binding",
)
'''
replacement = '''replace_exact(
    source,
    '    .bind(payload)\\n    .bind(transaction_id(invocation))\\n    .execute(&mut **transaction)\\n',
    '    .bind(payload)\\n    .bind(business_transaction_id)\\n    .execute(&mut **transaction)\\n',
    "record mutation business transaction bindings",
    expected=2,
)
'''
if text.count(insert_call) != 1 or text.count(update_call) != 1:
    raise SystemExit(
        "transaction binding materializer calls differ from the exact expected revision"
    )
text = text.replace(insert_call, replacement).replace(update_call, "")
exec(compile(text, str(path), "exec"), {"__name__": "__main__", "__file__": str(path)})
