# Credential placeholders

This folder is for local-only replacement files used by `deployment.json`.

Each placeholder in `deployment.json` has the form `{PLACEHOLDER_NAME}`. At deployment time, replace it with the contents of the matching local file:

- `{CORE_SERVER_PASSWORD}` -> `.credential/CORE_SERVER_PASSWORD`
- `{CORE_SERVER_ROOT_PASSWORD}` -> `.credential/CORE_SERVER_ROOT_PASSWORD`
- `{DEEPSEEK_API_KEY}` -> `.credential/DEEPSEEK_API_KEY`
- Hyper-V VM operator username -> `.credential/HYPERV_OPERATOR_USERNAME`
- Hyper-V VM operator password -> `.credential/HYPERV_OPERATOR_PASSWORD`

Do not commit replacement files. `.gitignore` ignores everything in this folder except this README.
