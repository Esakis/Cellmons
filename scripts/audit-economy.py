"""Seeded virtual-currency pacing model using canonical prices, pools and reward rules.

Registered, verified accounts only. No real money, trading, expeditions or tournament prizes.
The adaptive policy saves for the pack with the most expected useful copies per Ducat.
It immediately discards copies above the legal deck limit and recycles junkyard rewards.
This finite simulation is a design model, not observed player behavior or optimal play.
"""
from argparse import ArgumentParser
from fractions import Fraction
from pathlib import Path
from random import Random
import json
import tomllib

ROOT = Path(__file__).resolve().parent.parent


def percentile(values, percent):
    # Unreached milestones are censored above every observed day, not silently omitted.
    ordered = sorted(values, key=lambda v: (v is None, v if v is not None else 0))
    return ordered[(len(ordered) * percent + 99) // 100 - 1] if ordered else None


def career(content, offers, options, sample):
    rules = content['rules']
    rng = Random(0x18BCE735 ^ sample)
    wins_rng = Random(0x5BCA1836 ^ sample)
    cards = {card['id']: card for card in content['cards']}
    pools = {rarity: [card['id'] for card in cards.values()
                     if card['in_boosters'] and card['rarity'] == rarity]
             for rarity in ['bronze', 'silver', 'gold']}
    copies = {card: 0 for card in cards}
    starter = content['starters'][sample % len(content['starters'])]
    by_key = {card['key']: card['id'] for card in cards.values()}
    for key, qty in starter['cards'].items():
        copies[by_key[key]] = qty
    limit = rules['deck']['max_copies']
    incomplete = {r: sum(copies[c] < limit for c in pool) for r, pool in pools.items()}
    unique = sum(qty > 0 for qty in copies.values())
    complete = sum(qty >= limit for qty in copies.values())
    coins = rules['economy']['initial']
    income, recycled, spent, packs, points, rewards = 0, 0, 0, 0, 0, 0
    reached = {key: None for key in ['half_unique', 'all_unique', 'four_copies', 'all_junkyard']}
    rarity_points = dict(zip(['bronze', 'silver', 'gold'], rules['junkyard']['rarity_points']))
    junk_count = sum(a['rarity'] == 'junkyard' for a in content['avatars'])
    for day in range(options.max_days + 1):
        if day:
            wins = sum(wins_rng.randrange(10_000) < options.win_bps for _ in range(options.matches))
            award = ((rules['economy']['arena_first_win'] if wins else 0)
                     + min(wins * rules['economy']['arena_win'], rules['economy']['arena_win_daily_cap']))
            coins += award
            income += award
        for _ in range(100_000):
            if options.strategy == 'basic':
                offer = next(o for o in offers if o['id'] == 'basic')
            else:
                def value(offer):
                    return sum((Fraction(slot['count'] * incomplete[slot['rarity']], len(pools[slot['rarity']]))
                                for slot in offer['slots']), Fraction()) / offer['price']
                offer = max(offers, key=value)
            if coins < offer['price'] or complete == len(cards):
                break
            coins -= offer['price']
            spent += offer['price']
            packs += 1
            for slot in offer['slots']:
                rarity = slot['rarity']
                for _ in range(slot['count']):
                    card = rng.choice(pools[rarity])
                    if copies[card] < limit:
                        unique += copies[card] == 0
                        copies[card] += 1
                        if copies[card] == limit:
                            incomplete[rarity] -= 1
                            complete += 1
                    else:
                        points += rarity_points[rarity]
            while points >= rules['junkyard']['reward_threshold']:
                points -= rules['junkyard']['reward_threshold']
                if rewards >= junk_count:
                    coins += rules['junkyard']['fallback_ducats']
                    recycled += rules['junkyard']['fallback_ducats']
                rewards += 1
        else:
            raise ValueError('Unbounded recycling policy: review currency rules')
        milestones = {'half_unique': unique * 2 >= len(cards), 'all_unique': unique == len(cards),
                      'four_copies': complete == len(cards), 'all_junkyard': rewards >= junk_count}
        for key, done in milestones.items():
            if done and reached[key] is None:
                reached[key] = day
        if complete == len(cards):
            break
    assert coins == rules['economy']['initial'] + income + recycled - spent and coins >= 0
    return {'days': reached, 'packs': packs, 'earned': income, 'recycled': recycled,
            'spent': spent, 'remaining': coins, 'completed': complete == len(cards)}


def main():
    parser = ArgumentParser(description=__doc__)
    parser.add_argument('--samples', type=int, default=100)
    parser.add_argument('--matches', type=int, default=20)
    parser.add_argument('--win-bps', type=int, default=5000)
    parser.add_argument('--max-days', type=int, default=5000)
    parser.add_argument('--strategy', choices=['adaptive', 'basic'], default='adaptive')
    options = parser.parse_args()
    if not (1 <= options.samples <= 10_000 and 1 <= options.matches <= 1000
            and 0 <= options.win_bps <= 10_000 and 1 <= options.max_days <= 100_000):
        parser.error('Options outside model limits')
    content = json.loads((ROOT / 'target/content/content.json').read_text(encoding='utf8'))
    offers = tomllib.loads((ROOT / 'content/boosters.toml').read_text(encoding='utf8'))['booster']
    assert all(offer['price'] > 0 for offer in offers)
    rows = [career(content, offers, options, sample) for sample in range(options.samples)]
    report = {'content_version': content['version'], 'assumptions': vars(options),
              'completed': sum(row['completed'] for row in rows), 'milestones': {}}
    for key in rows[0]['days']:
        values = [row['days'][key] for row in rows]
        report['milestones'][key] = {'reached': sum(v is not None for v in values),
                                   'p10_days': percentile(values, 10), 'p50_days': percentile(values, 50),
                                   'p90_days': percentile(values, 90)}
    for key in ['packs', 'earned', 'recycled', 'spent', 'remaining']:
        report[f'median_{key}'] = percentile([row[key] for row in rows], 50)
    path = ROOT / f'target/economy-audit-{options.strategy}.json'
    path.write_text(json.dumps(report, indent=2) + '\n', encoding='utf8', newline='\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
