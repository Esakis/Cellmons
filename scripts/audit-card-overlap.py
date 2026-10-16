"""Report simple numeric overlap candidates for manual content review.

This deliberately excludes drain, hand operations, swaps, attack filters, school changes,
and other context-dependent families. A candidate is not a proof of game-theoretic dominance.
No source content is modified, and the report is not used to certify global balance.
"""
from collections import Counter
from pathlib import Path
import json
import tomllib

ROOT = Path(__file__).resolve().parent.parent


def vector(card):
    result = Counter()
    for effect in card['effects']:
        family, target = effect['family'], effect.get('target')
        if family not in {'heal', 'damage', 'stamina', 'focus', 'stat_mod'} or target == 'both':
            return None
        sign = 1 if target == 'self' else -1
        if family == 'damage':
            sign = -sign
        key = family, target, effect.get('stat'), effect.get('duration', 'duel')
        result[key] += sign * effect['amount']
    return result


def candidates(cards):
    for a in cards:
        va = vector(a)
        if va is None:
            continue
        for b in cards:
            if a['id'] == b['id'] or a['kind'] != b['kind']:
                continue
            if any(a.get(k) != b.get(k) for k in ['condition', 'coupling', 'trigger']):
                continue
            if set(a.get('keywords', [])) != set(b.get('keywords', [])):
                continue
            ar, br = a.get('requires', {}), b.get('requires', {})
            if any(level > br.get(school, 0) for school, level in ar.items()):
                continue
            if any(a['cost'][k] > b['cost'][k] for k in ['focus', 'stamina']):
                continue
            vb = vector(b)
            if vb is None:
                continue
            keys = va.keys() | vb.keys()
            if all(va[k] >= vb[k] for k in keys) and (
                any(va[k] > vb[k] for k in keys) or a['cost'] != b['cost'] or ar != br
            ):
                yield {'stronger': a['key'], 'review': b['key']}


def main():
    cards = [card for path in sorted((ROOT / 'content/cards').rglob('*.toml'))
             for card in tomllib.loads(path.read_text(encoding='utf8'))['card']]
    report = list(candidates(cards))
    (ROOT / 'target').mkdir(exist_ok=True)
    (ROOT / 'target/card-overlap.json').write_text(
        json.dumps(report, indent=2) + '\n', encoding='utf8', newline='\n')
    print(f'{len(cards)} cards, {len(report)} numeric overlap candidates requiring contextual review.')
    for pair in report:
        print(f'{pair["stronger"]} -> {pair["review"]}')


if __name__ == '__main__':
    main()
