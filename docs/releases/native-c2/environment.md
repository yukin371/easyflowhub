### C2 environment

Collected: `2026-11-25 14:08:42`

| Item | Value |
|---|---|
| OS | Microsoft Windows 11 专业工作站版 build **26300** (64 位) |
| Session | `Console` (local) |
| User | yukin |
| Displays (WMI monitor entries) | 2 |
| Adapter `AMD Radeon(TM) Graphics` | 3840x2160 |

**Input methods under test** (identity is the TIP CLSID; version comes from the service binary):

| Input method | TIP CLSID | Enabled | Version | Binary |
|---|---|---|---|---|
| Microsoft Pinyin | `{81d4e9c9-1d3b-41bc-9e6c-4b40bf79e35e}` | **yes** | n/a | inbox service (no InprocServer32) |
| Sogou Pinyin (搜狗拼音输入法) | `{e7ea138e-69f8-11d7-a6ea-00065b844310}` | **yes** | 16.9.0.5795 | SogouTSF.ime |
| WeChat input (WeType) | `{86598fb9-66a2-463e-b9c2-aeb906d477ad}` | **yes** | 1.4.2.12 | wetype_tip.dll |

**Other zh-CN text services registered on this machine** (not under test):

- Microsoft IME -- `{03b5835f-f03c-411b-9ce2-aa23e1171e36}`
- Microsoft Changjie -- `{531fdebf-9b4c-4a43-a2aa-960e8fcdc732}`
- Microsoft Wubi -- `{6a498709-e00b-4c45-a018-8f9e4081ae40}`
- Amharic Input Method 2 -- `{7c472071-36a7-4709-88cc-859513e583a9}`
- Hindi Phonetic -- `{81ea0a17-aa39-455b-ba20-ea79a8f98966}`
- Tablet PC Correction -- `{8613e14c-d0c0-4161-ac0f-1dd2563286bc}`
- Microsoft IME -- `{a028ae76-01b1-46c2-99c4-acd9858ae02f}`
- Microsoft Old Hangul IME -- `{a1e2b86b-924a-4d43-80f6-8a820df7190f}`
- Microsoft Bopomofo -- `{b115690a-ea02-48d5-a231-e3578d2fdf80}`
- Vietnamese Number Key-Based -- `{c2cb2cf0-af47-413e-9780-8bc3a3c16068}`
- Speech Recognition -- `{dcbd6fa8-032f-11d3-b5b1-00c04fc324a1}`
- Chinese Traditional DaYi -- `{e429b25a-e5d3-4d1f-9be3-0c608477e3a1}`
- Tablet PC Text Insertion -- `{f25e9f57-2fc8-4eb3-a41a-cce5f08541e6}`
- Ink Correction -- `{fa445657-9379-11d6-b41a-00065b83ee53}`

**Binary under test:**

- Path: `E:\Github\AllScripts\easyflowhub\native\target\release\easyflowhub-native.exe`
- Built: `2026-11-25 12:22:23`  Size: `6.1 MB`
- SHA256: `7e65a15c7f0d639b4224f3c0a61f37dfec8da6634ec18d744f86cd00cd9cfe2a`

**Product processes running right now:**

- `easyflowhub-native` pid 3808, 194.7 MB

**Readiness for C2 (needs Microsoft Pinyin + Sogou + WeChat):**

- Microsoft Pinyin: present and enabled
- Sogou: present and enabled
- WeChat input: present and enabled

> DPI / scale is not guessed here on purpose: take native_ppp= from the startup log line set_ppp,
> which is the value the panel was actually laid out at.
