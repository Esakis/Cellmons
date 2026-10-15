"""Remove optional fraction-only glyphs unused by the UI; keep all existing Unicode characters.

Authoring tool only: fontTools 4.67.0 and Brotli 1.2.0, as with the original subsets.
No Python dependency is needed for ordinary application builds.
"""
from pathlib import Path
from fontTools import subset
from fontTools.ttLib import TTFont

ROOT=Path(__file__).resolve().parent.parent

def main():
    path=ROOT/'web/src/styles/fonts/inter-latin.woff2'
    original=path.read_bytes();backup=ROOT/'target/inter-latin-before-subset.woff2'
    if not backup.exists():backup.write_bytes(original)
    font=TTFont(path);characters=set(font.getBestCmap());features=set()
    for tag in ['GSUB','GPOS']:
        if tag in font:features.update(r.FeatureTag for r in font[tag].table.FeatureList.FeatureRecord)
    options=subset.Options();options.flavor='woff2';options.layout_features=sorted(features-{'frac','numr','dnom'});options.name_IDs=['*'];options.name_languages=['*'];options.hinting=True
    subsetter=subset.Subsetter(options=options);subsetter.populate(unicodes=characters);subsetter.subset(font)
    assert set(font.getBestCmap())==characters
    font.flavor='woff2';font.save(path)
    print(f'Inter Latin: {len(original)} -> {path.stat().st_size} bytes, unchanged Unicode coverage; fraction features removed.')

if __name__=='__main__':main()
