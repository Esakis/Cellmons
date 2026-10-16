"""Authoring only: merge the archived Cinzel 600 subsets with fontTools 4.67.0/Brotli 1.2.0."""
from pathlib import Path
from fontTools.merge import Merger

ROOT=Path(__file__).resolve().parent.parent
font=Merger().merge([str(ROOT/f'art/font-sources/cinzel-{group}.woff2') for group in ['latin','latin-ext']])
font.flavor='woff2'
font.save(ROOT/'web/src/styles/fonts/cinzel.woff2')
