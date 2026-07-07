# Credentials

Local-only secret files used by Marix. This folder is git-ignored except for
this README; never commit the replacement files.

## Runtime config references

`config.toml` refers to secrets by name via `{ name = "..." }`, resolved against
this folder as `<name>.txt`:

- `[model.deepseek] api_key = { name = "DEEPSEEK_API_KEY" }` -> `DEEPSEEK_API_KEY.txt`

## Deployment secrets

Consumed by deployment and Hyper-V operator tooling, not by the runtime config:

- `CORE_SERVER_PASSWORD.txt`
- `CORE_SERVER_ROOT_PASSWORD.txt`
- `CORE_SERVER_SSH_KEY.txt` (path to the SSH private key for the Ubuntu core host)
- `HYPERV_OPERATOR_USERNAME.txt`
- `HYPERV_OPERATOR_PASSWORD.txt`
