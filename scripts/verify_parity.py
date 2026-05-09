#!/usr/bin/env python3
"""算法对齐校验：把 pathy-knowledge-server 的核心算法输出冻结成 JSON "金标准"，
新 Rust 后端用同一份 fixture 计算后比对。

目的：保证 BM25 / query terms / Markdown 切块 / 思考块剥离 / 召回融合排序 / 向量索引
等所有核心数据结构与 Python 参考实现 bit-for-bit 一致。

用法：
  # 1) 在 pathy-knowledge-server 项目 venv 下跑：
  cd ../pathy-knowledge-server && source .venv/bin/activate
  python ../reef-knowledge/scripts/verify_parity.py --emit-golden /tmp/parity.json

  # 2) 在新项目下跑 Rust 端的 verify_parity（cargo test）比对：
  cd ../reef-knowledge/src-tauri && cargo test --test parity_test \\
    -- --nocapture --golden /tmp/parity.json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

# 把 pathy-knowledge-server 的 app 模块加入路径
HERE = Path(__file__).resolve()
PATHY_SERVER = HERE.parents[2] / "pathy-knowledge-server"
if str(PATHY_SERVER) not in sys.path:
    sys.path.insert(0, str(PATHY_SERVER))

from app.services.dialogue_recall import (  # noqa: E402
    _extract_query_terms,
    _filter_terms,
    _is_meaningful_body,
    _markdown_heading_present,
    _parse_md_sections,
    _score_chunks_bm25,
    _split_chunks,
    _wiki_indexed_chunks,
    _IndexedChunk,
)
from app.services.llm_tasks import _strip_think_blocks  # noqa: E402
from app.services.recall_stopwords import DEFAULT_STOPWORDS  # noqa: E402


SAMPLE_QUERIES = [
    "如何配置 OpenAI API base_url 和 model 参数？",
    "What is BM25?",
    "向量召回的 top_k_chunks 怎么调整",
    "",  # 边界
    "the and or",  # 全是停用词
    "RAG 与 wiki 编译流程",
]

SAMPLE_MARKDOWN = """\
# 标题一

正文一段，不含标题之外内容。

## 子标题

- 列表项 A
- 列表项 B

---

# 标题二

```python
print("hello")
```

正文二
"""

SAMPLE_TEXT_NO_HEADING = """\
没有标题的纯文本。

第二段，应当作为单独的滑窗块。

第三段。
""" * 30  # 故意拉长，触发滑窗

SAMPLE_THINK_BLOCKS = (
    "前缀<think>这是隐藏推理</think>正文一<reasoning attr='x'>多行\n推理</reasoning>正文二\n"
    "<思考>中文思考块</思考>结尾"
)


def to_chunk_dict(c: _IndexedChunk) -> dict:
    return {"rel": c.rel, "heading_path": c.heading_path, "body": c.body}


def emit_golden(out_path: Path) -> None:
    golden: dict = {"version": 1}

    # 1. query terms
    golden["query_terms"] = []
    for q in SAMPLE_QUERIES:
        raw = _extract_query_terms(q)
        kept = _filter_terms(raw, set(DEFAULT_STOPWORDS))
        golden["query_terms"].append({"query": q, "raw": raw, "kept": kept})

    # 2. markdown 处理
    golden["markdown_heading_present"] = _markdown_heading_present(SAMPLE_MARKDOWN)
    golden["md_sections"] = _parse_md_sections(SAMPLE_MARKDOWN)
    golden["chunking_with_filter"] = [
        to_chunk_dict(c)
        for c in _wiki_indexed_chunks("doc.md", SAMPLE_MARKDOWN, 1200)
    ]

    # 3. 滑窗切块（无标题）
    golden["split_no_heading"] = _split_chunks(SAMPLE_TEXT_NO_HEADING, 1200)

    # 4. is_meaningful_body 边界
    golden["is_meaningful_body"] = {
        "empty": _is_meaningful_body(""),
        "blank": _is_meaningful_body("   \n  "),
        "rule_only": _is_meaningful_body("---"),
        "stars_only": _is_meaningful_body("***\n___"),
        "real": _is_meaningful_body("实际正文"),
    }

    # 5. think blocks 剥离
    golden["strip_think"] = _strip_think_blocks(SAMPLE_THINK_BLOCKS)

    # 6. BM25 评分
    chunks = _wiki_indexed_chunks("doc.md", SAMPLE_MARKDOWN, 1200)
    terms = ["标题", "正文", "列表"]
    bm25 = _score_chunks_bm25(chunks, terms)
    golden["bm25_scores"] = [
        {"score": round(score, 6), **to_chunk_dict(c)} for score, c in bm25
    ]

    out_path.write_text(
        json.dumps(golden, ensure_ascii=False, indent=2, sort_keys=False),
        encoding="utf-8",
    )
    print(f"[ok] golden written: {out_path}")
    print(json.dumps(golden, ensure_ascii=False, indent=2)[:600] + "\n...")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--emit-golden", type=Path, required=True)
    args = ap.parse_args()
    emit_golden(args.emit_golden)
    return 0


if __name__ == "__main__":
    sys.exit(main())
