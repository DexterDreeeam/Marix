# `.credential/` — git-crypt encrypted secrets

Every `*.txt` file in this directory is git-crypt ciphertext, not plaintext.
The decryption key is kept one directory level above the repository
workspace root, at `<repo_workspace>\..\marix-git-crypt.key`. To decrypt,
run from the repository root:

```powershell
git-crypt unlock ..\marix-git-crypt.key
```
