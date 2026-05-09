#!/usr/bin/env python3
"""为 Tauri 2 Android 工程打补丁，解决软键盘弹起时遮挡 WebView 输入区的问题。

用法（在执行 `npm run tauri android init` 之后运行一次）：

    python3 scripts/patch_android_manifest.py

效果：

1. AndroidManifest.xml
   - 在 `<activity>` 上加 `android:windowSoftInputMode="adjustResize"`，
     在 SDK 34 及以下让 WebView 在键盘弹起时自动 resize，CSS `100dvh` /
     `position: fixed; bottom: 0` 会自动跟随键盘上方。

2. MainActivity.kt（SDK 35+ Edge-to-Edge 兜底）
   - 监听 `WindowInsetsCompat`，把 IME（键盘）高度合并到 root view 的 padding-bottom，
     从而模拟"adjustResize"行为；不影响 SDK 34 及以下。
   - 参考 tauri issue #10631 中 @wanglinshen2021 的方案。

脚本是幂等的：重复运行不会重复打补丁。
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
ANDROID_DIR = REPO_ROOT / "src-tauri" / "gen" / "android"


def patch_manifest() -> bool:
    """给 AndroidManifest.xml 的主 Activity 加 windowSoftInputMode=adjustResize。"""
    manifest_paths = list(ANDROID_DIR.glob("app/src/main/AndroidManifest.xml"))
    if not manifest_paths:
        print(f"[skip] 未找到 AndroidManifest.xml（请先 `npm run tauri android init`）：{ANDROID_DIR}")
        return False
    manifest = manifest_paths[0]

    text = manifest.read_text(encoding="utf-8")
    if "windowSoftInputMode" in text:
        print(f"[ok] AndroidManifest.xml 已包含 windowSoftInputMode，跳过：{manifest}")
        return True

    # 在主 <activity ... > 标签的属性集中插入 windowSoftInputMode="adjustResize"
    pattern = re.compile(r'(<activity\b[^>]*?android:name="\.MainActivity"[^>]*?)(>)', re.S)

    def _inject(match: re.Match[str]) -> str:
        head, gt = match.group(1), match.group(2)
        return f'{head}\n            android:windowSoftInputMode="adjustResize"{gt}'

    new_text, n = pattern.subn(_inject, text, count=1)
    if n == 0:
        # 退化策略：尝试匹配任意 <activity ...>
        pattern2 = re.compile(r"(<activity\b[^>]*?)(>)", re.S)
        new_text, n = pattern2.subn(_inject, text, count=1)

    if n == 0:
        print(f"[warn] 未在 AndroidManifest.xml 中找到 <activity> 标签，已跳过：{manifest}")
        return False

    manifest.write_text(new_text, encoding="utf-8")
    print(f"[ok] 已为 AndroidManifest.xml 添加 windowSoftInputMode=adjustResize：{manifest}")
    return True


WINDOW_INSETS_HOOK = """
        // —— REEF-CC patch: SDK 35+ Edge-to-Edge 下手动处理 IME（软键盘）insets —— //
        // 参考 tauri-apps/tauri#10631；SDK 34 及以下走 windowSoftInputMode=adjustResize 即可。
        try {
            val root: android.view.View = window.decorView
            androidx.core.view.ViewCompat.setOnApplyWindowInsetsListener(root) { v, insets ->
                val sys = insets.getInsets(androidx.core.view.WindowInsetsCompat.Type.systemBars())
                val ime = insets.getInsets(androidx.core.view.WindowInsetsCompat.Type.ime())
                v.setPadding(sys.left, sys.top, sys.right, kotlin.math.max(sys.bottom, ime.bottom))
                androidx.core.view.WindowInsetsCompat.CONSUMED
            }
        } catch (_: Throwable) { /* 老版本设备静默回退 */ }
        // —— end patch —— //
"""


def patch_main_activity() -> bool:
    """在 MainActivity.kt 的 onCreate 末尾注入 WindowInsets 监听（SDK 35+ 兜底）。"""
    candidates = list(ANDROID_DIR.glob("app/src/main/java/**/MainActivity.kt"))
    if not candidates:
        print(f"[skip] 未找到 MainActivity.kt：{ANDROID_DIR}")
        return False
    main_activity = candidates[0]

    text = main_activity.read_text(encoding="utf-8")
    if "REEF-CC patch" in text:
        print(f"[ok] MainActivity.kt 已注入 WindowInsets 监听，跳过：{main_activity}")
        return True

    # 找到 onCreate 函数体并在内部末尾追加注入代码。
    # 兼容形如：
    #   override fun onCreate(savedInstanceState: Bundle?) {
    #       super.onCreate(savedInstanceState)
    #       ...
    #   }
    pattern = re.compile(
        r"(override\s+fun\s+onCreate\s*\([^)]*\)\s*\{[^{}]*?)(\n\s*\})",
        re.S,
    )
    new_text, n = pattern.subn(rf"\1{WINDOW_INSETS_HOOK}\2", text, count=1)
    if n == 0:
        # 没有 onCreate（极少见）：在 class 末尾插入一个完整的 onCreate
        class_pattern = re.compile(r"(class\s+MainActivity[^{]*\{)", re.S)

        def _inject(match: re.Match[str]) -> str:
            head = match.group(1)
            return (
                f"{head}\n"
                "    override fun onCreate(savedInstanceState: android.os.Bundle?) {\n"
                "        super.onCreate(savedInstanceState)\n"
                f"{WINDOW_INSETS_HOOK}"
                "    }\n"
            )

        new_text, n = class_pattern.subn(_inject, text, count=1)

    if n == 0:
        print(f"[warn] 未能定位 MainActivity 注入点，已跳过：{main_activity}")
        return False

    main_activity.write_text(new_text, encoding="utf-8")
    print(f"[ok] 已为 MainActivity.kt 注入 WindowInsets 监听：{main_activity}")
    return True


def main() -> int:
    if not ANDROID_DIR.exists():
        print(
            f"[error] Android 工程目录不存在：{ANDROID_DIR}\n"
            "        请先运行 `npm run tauri android init` 生成移动端工程。",
            file=sys.stderr,
        )
        return 1

    ok1 = patch_manifest()
    ok2 = patch_main_activity()
    return 0 if (ok1 or ok2) else 1


if __name__ == "__main__":
    sys.exit(main())
