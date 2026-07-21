# `.credential/` —— git-crypt 加密的凭据文件

本目录下所有 `*.txt` 文件都是 git-crypt 密文，而不是明文。解密密钥固定存放在
仓库工作区根目录的上一级目录，即 `<repo_workspace>\..\marix-git-crypt.key`。
在仓库根目录下执行以下命令即可解锁：

```powershell
git-crypt unlock ..\marix-git-crypt.key
```
