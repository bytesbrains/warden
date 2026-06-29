# Building the Warden whitepaper PDF

## Sources

- **`whitepaper.md`** — the **canonical source**. Edit content here.
- **`whitepaper.tex`** — generated from `whitepaper.md` by pandoc (do not hand-edit; regenerate).

## ⚠️ Not yet compiled / verified

The `.tex` was generated but **has not been compiled in this environment** (no TeX engine is installed here). **Test-compile it before posting** — on Overleaf (easiest) or locally with a TeX Live install. Treat the current `.tex` as a first cut that may need small fixes on first compile.

## Compile

Use **XeLaTeX or LuaLaTeX — not pdfLaTeX.** The construction pseudocode uses Unicode symbols (σ, ‖, ·, ⌈ ⌉, →) that pdfLaTeX cannot typeset in verbatim. The preamble auto-selects: under XeLaTeX/LuaLaTeX it loads `fontspec`/`unicode-math` and sets the mono font to **DejaVu Sans Mono** (which covers those glyphs).

**Easiest — Overleaf:**
1. New Project → Upload `whitepaper.tex`.
2. Menu → Compiler → **XeLaTeX**.
3. Recompile → download PDF. (Overleaf ships DejaVu Sans Mono.)

**Local (TeX Live with `texlive-fonts-extra` for DejaVu):**
```sh
xelatex whitepaper.tex
xelatex whitepaper.tex   # second pass for cross-references
```
(or `latexmk -xelatex whitepaper.tex`)

If DejaVu Sans Mono is unavailable, either install it or change the `\setmonofont{...}` line to any installed Unicode-covering mono font.

## Regenerate `whitepaper.tex` after editing `whitepaper.md`

```sh
# from publication/warden-whitepaper/
cat > /tmp/wp-yaml.md <<'YAML'
---
title: "Warden: Event-Gated Threshold Conditional-Decryption for Guaranteed-but-Deferred Encrypted Delivery"
author:
  - "Sandeep Nandal\\thanks{BytesBrains Pte Ltd, \\texttt{sandeep@nandal.in}. Solo authorship reflects the work as done; a co-author position is open to an academic collaborator who contributes to the formal peer-reviewed version.}"
date: "Draft v1 · June 2026"
---

YAML
# body starts at the Status disclosure (YAML supplies title/author/date; drop the .md header block)
awk '/^\*\*Status disclosure/{p=1} p' whitepaper.md > /tmp/wp-body.md
cat /tmp/wp-yaml.md /tmp/wp-body.md > /tmp/wp-src.md
pandoc /tmp/wp-src.md -s -o whitepaper.tex \
  -V documentclass=article -V geometry:margin=1in -V fontsize=11pt \
  -V colorlinks=true -V linkcolor=blue -V urlcolor=blue \
  -V monofont="DejaVu Sans Mono" \
  --shift-heading-level-by=-1
```

## Where to post

The paper is **self-hosted** — see [`README.md`](README.md) for the full publishing handoff. In short:

- **Warden repo** (https://github.com/bytesbrains/warden) under `docs/` — `whitepaper.md` renders inline; ship the recompiled `whitepaper.pdf` alongside.
- **Landing page** (https://warden.bytesbrains.com) — link the **PDF** with the status-disclosure blurb from `README.md`.

No archive submission (IACR ePrint / arXiv was considered and dropped). A formal peer-reviewed venue (PETS / FC / ACNS) is a later, optional step gated on an academic co-author.

## Known follow-ups (only if a formal peer-reviewed version is later pursued)

- The reference list is a plain itemized list, not BibTeX. Convert to a `.bib` + `\cite{}` (fine as-is for the self-hosted paper).
- In-text section references are literal (`§6.1`). A formal version would switch to `\label`/`\Cref` so they auto-number.
- Section headings carry manual numbers ("1. Introduction"); auto-numbering is disabled so they don't double up. If you switch to `\Cref`, re-enable `secnumdepth` and drop the manual numbers.
</content>
