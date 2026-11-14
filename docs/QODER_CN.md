# Qoder CN platform

Qoder and Qoder CN are independent platforms, following the existing CodeBuddy / CodeBuddy CN convention. There is no region selector inside the account dialog.

| | Qoder | Qoder CN |
|---|---|---|
| Platform ID | `qoder` | `qoder_cn` |
| Page | `qoder` | `qoder-cn` |
| Account details | `qoder_accounts/` | `qoder_cn_accounts/` |
| Typed account index | `QoderAccountIndex` | `QoderCnAccountIndex` |
| Device API | `openapi.qoder.sh` | `openapi.qoder.com.cn` |
| Sign-in | `qoder.com` | `qoder.cn` |
| Native desktop profile | `com.qoder.app.stable` | `com.qodercn.app.stable` |
| CLI profile | `.qoder/.auth` | `.qoder-cn/.auth` |

Frontend stores, current-account references, groups, persisted filters, automatic refresh settings and account transfers use the corresponding platform ID. Existing global IDE switching and dedicated deployment support are retained. Shared Rust implementations keep token renewal and account persistence consistent, with separate module statics and typed SQLite indexes per platform.

Qoder CN supports device authorization, JSON/native import, quota refresh, and switching the default native desktop or CLI client. Windows desktop credentials use the client's DPAPI-protected AES-GCM key. CLI credentials use the installed client's existing machine ID. The client must be closed before switching; Cockpit backs up the existing profile, writes atomically, verifies the result, and restores it if verification fails. Reopen the target client after switching. CN multiple instances are not implemented.

Credentials marked `qodercn` cannot be imported or refreshed through the global platform. Credentials are never tried against another region's endpoint. CN requests do not reuse global IDE machine tokens. Refresh-token rotation is persisted before subsequent quota requests, preserving upstream's renewal behavior. The existing `dedicated_qodercn` deployment remains in its upstream IDE platform; it is not reclassified as the public CN service.

Native profile and device-login conventions are adapted from [WorkBuddy Switch Next](https://github.com/coderDJing/workbuddy-switch-next), licensed under MIT. Attribution is included in `resources/licenses/WorkBuddy-Switch-Next.txt` and the application bundle.

The private fork additionally provides `--fork-cli status`, `self-test`, `import-native <cn|global> <desktop|cli>`, `refresh <cn|global> <id>`, and `migrate-cn` for local migration. Output excludes credentials. `migrate-cn` copies a legacy CN record to the CN store and verifies it before removing its global entry; existing native client profiles are not changed by migration.
