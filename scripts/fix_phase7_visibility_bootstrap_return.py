from pathlib import Path

path = Path("crates/crm-application-runtime/src/runtime.rs")
text = path.read_text()
old = """        })
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))
}

fn expiry(now_unix_nanos: i64) -> Result<i64, ApplicationRuntimeError> {
"""
new = """        })
        .map(|_| ())
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))
}

fn expiry(now_unix_nanos: i64) -> Result<i64, ApplicationRuntimeError> {
"""
count = text.count(old)
if count != 1:
    raise RuntimeError(f"visibility bootstrap return anchor: expected 1, found {count}")
path.write_text(text.replace(old, new, 1))
