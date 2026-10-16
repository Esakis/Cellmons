"""Original SVG enchantment emblems, drawn from the new Relic card themes."""
from pathlib import Path
from html import escape
import tomllib

ROOT = Path(__file__).resolve().parent.parent
PALETTE = {'combat':('#652b30','#e6ae80'),'divine':('#67512a','#ffe3a0'),'element':('#245458','#a5e6cf'),'arcane':('#36305d','#cabffa'),'necro':('#3e2c49','#d8b7e3'),'tech':('#22535c','#a7e7df')}
DRAWINGS = {
'chronicle_veil':'<path d="M33 12Q80 31 127 12L116 87Q80 68 44 87Z"/><path d="M59 31H100M59 68H100M63 32L97 67M97 32L63 67" fill="none"/><circle cx="80" cy="49" r="5"/>',
'grave_halo':'<ellipse cx="80" cy="22" rx="39" ry="13" fill="none"/><path d="M49 63Q49 34 80 35T111 63L104 85H56Z"/><circle cx="67" cy="59" r="7" fill="var(--dark)"/><circle cx="93" cy="59" r="7" fill="var(--dark)"/><path d="M75 75L80 66 85 75M67 84V92M80 84V94M94 84V92" fill="none"/>',
'dawn_embodiment':'<circle cx="80" cy="36" r="17"/><path d="M80 54L65 76 80 91 96 76ZM52 49L25 73 62 67ZM109 49L136 73 100 67Z"/><path d="M80 7V14M43 14L51 23M117 14L109 23M29 40H43M117 40H132" fill="none"/>',
'steadfast_blessing':'<path d="M43 24L80 12 119 24V59Q103 83 80 92 57 82 43 59Z"/><path d="M80 30V70M62 48H98" fill="none" stroke="var(--dark)" stroke-width="7"/><path d="M25 53L17 43M134 52L142 43M56 7L51 1M104 7L110 1" fill="none"/>',
'essence_road':'<path d="M34 94Q117 66 74 47T86 6M65 95Q145 66 103 44T112 4" fill="none" stroke-width="7"/><path d="M88 27L94 18 102 25 96 36ZM62 68L72 62 80 71 68 79Z"/>',
'sword_oath':'<path d="M72 65V30L80 6 88 30V65M51 67H108M73 70V89H87V70"/><path d="M34 28Q12 51 40 79M126 28Q148 51 120 79M36 29L34 47M124 29L127 47" fill="none"/>',
'living_elixir':'<path d="M64 12H96V36L115 61V81Q80 100 45 81V61L64 36Z"/><path d="M51 63Q80 50 109 63V78Q81 91 51 78Z" fill="var(--dark)"/><path d="M68 20H92M80 65V81M72 73H88" fill="none"/><circle cx="72" cy="48" r="3"/>',
'wild_current':'<path d="M20 31Q49 12 77 35T140 30M20 54Q50 34 78 57T141 51M20 78Q48 59 78 80T140 74" fill="none" stroke-width="8"/><path d="M68 26L86 12 101 27 85 43Z"/>',
'calming_regulator':'<circle cx="80" cy="50" r="32"/><circle cx="80" cy="50" r="21" fill="var(--dark)"/><path d="M79 50L98 34M80 9V18M80 82V92M39 50H24M122 50H136M49 19L39 9M111 81L121 91" fill="none"/><circle cx="80" cy="50" r="5"/>',
'sage_stone':'<path d="M80 7L113 27 102 68 80 88 57 67 47 27Z"/><path d="M80 7L69 39 80 88 92 39ZM48 27L69 39 111 27M57 67L69 39 102 68" fill="var(--dark)"/><ellipse cx="80" cy="90" rx="44" ry="5" fill="none"/>',
'fury_catalyst':'<path d="M54 14H105V28L91 46 113 73 102 89H57L46 74 68 46 54 28Z"/><path d="M64 22H95M68 66L81 50 92 70 80 81Z" fill="var(--dark)"/><path d="M29 30L42 40M130 29L117 41M22 63H38M123 63H140" fill="none"/>',
'runic_workshop':'<path d="M25 88V51L50 31 74 51 103 31 135 51V88Z"/><path d="M45 30V11H60V39M111 32V9H124V42"/><path d="M42 64H54V78H42ZM72 64H84V78H72ZM103 64H117V78H103Z" fill="var(--dark)"/>',
'radiant_scepter':'<path d="M76 42V94H84V42M54 20L65 41 80 48 97 41 108 20 91 28 81 8 71 28Z"/><circle cx="80" cy="32" r="8" fill="var(--dark)"/><path d="M39 10L49 20M121 10L112 20M32 37H46M114 37H130" fill="none"/>',
'astral_scepter':'<path d="M77 43V94H84V43M80 5L89 20 107 23 93 37 96 55 80 47 64 55 67 37 53 23 72 20Z"/><circle cx="80" cy="29" r="7" fill="var(--dark)"/><path d="M36 30Q80 0 124 30T36 30" fill="none"/>',
'verdant_staff':'<path d="M79 94L77 47Q38 53 45 24 74 20 79 45 85 9 116 13 120 45 83 48"/><path d="M53 31L79 49 107 23" fill="none"/><path d="M81 69Q105 50 116 65 105 81 81 76M79 80Q57 61 44 77 58 89 79 87"/>',
'martial_staff':'<path d="M43 90L101 16 108 22 51 96Z"/><path d="M85 18L98 4 119 23 106 37Z"/><path d="M88 23L108 25M42 74L64 89M47 67L70 82" fill="none"/><path d="M97 35Q125 45 139 30M88 46Q118 62 132 50" fill="none"/>',
'mending_halo':'<ellipse cx="80" cy="23" rx="44" ry="15" fill="none" stroke-width="5"/><path d="M39 64Q47 45 68 53L81 64 95 53Q118 44 125 65 129 82 81 95 37 83 39 64Z"/><path d="M80 64V84M69 74H91" fill="none" stroke="var(--dark)"/>',
'night_rite':'<path d="M37 88H125L110 69H53ZM79 12Q59 41 76 56 107 58 100 32 91 44 90 40Z"/><path d="M56 64L80 53 105 64M30 39L18 28M129 39L142 28" fill="none"/><circle cx="45" cy="17" r="3"/>',
'ember_rain':'<path d="M38 11L57 36 48 55 31 51 23 35ZM88 6L107 28 97 47 80 41 73 24ZM125 45L143 66 134 85 117 80 111 64Z"/><path d="M21 84L72 54M49 94L92 65M103 41L125 24" fill="none"/>',
'lumen_optimizer':'<path d="M58 13H102V33L120 58V84H40V58L58 33Z"/><path d="M68 21H92M47 66H113M80 44V61M71 52H89" fill="none"/><circle cx="58" cy="77" r="4" fill="var(--dark)"/><circle cx="80" cy="77" r="4" fill="var(--dark)"/><circle cx="102" cy="77" r="4" fill="var(--dark)"/>',
'patient_commander':'<path d="M48 83L57 65V39H103V65L113 83Z"/><path d="M61 37L56 22 66 10 80 22 94 10 106 22 99 37Z"/><path d="M80 42V67M67 55H94M36 88H125" fill="none"/>',
'sable_exchange':'<path d="M25 36Q58 8 97 28L109 39M101 17L109 39 87 41M134 64Q102 92 65 73L51 62M58 85L50 63 75 60" fill="none" stroke-width="5"/><path d="M44 38L57 52 44 67 31 52ZM112 34L125 48 112 63 99 48Z"/>',
'aether_field':'<ellipse cx="80" cy="52" rx="57" ry="31" fill="none"/><ellipse cx="80" cy="52" rx="28" ry="43" fill="none"/><path d="M24 52H136M79 9V94" fill="none"/><path d="M80 30L99 52 80 73 61 52Z"/>',
'sable_curse':'<path d="M43 21L80 39 116 20 107 62 80 89 52 62Z"/><path d="M59 42L72 50M89 50L102 42M65 68L80 59 96 68M80 40V59" fill="none"/><path d="M25 15L38 30M135 15L123 30M34 83L48 73M126 83L113 73" fill="none"/>',
'dune_mantle':'<path d="M17 74Q52 26 80 57 104 15 144 65L142 89H19Z"/><path d="M25 78Q65 44 80 66 114 40 137 74M34 87Q91 66 129 84" fill="none"/><circle cx="49" cy="24" r="13"/>',
'sword_dancer':'<circle cx="83" cy="24" r="9"/><path d="M79 35L94 47 80 62 108 87M81 59L57 84M82 43L58 45 38 31M91 47L111 33 125 38" fill="none" stroke-width="6"/><path d="M26 21L42 29 26 49 19 46ZM127 24L139 15 133 48 126 46Z"/>',
'temporal_hollow':'<path d="M69 10A40 40 0 1 0 69 90L52 72 64 55 51 39ZM92 10A40 40 0 0 1 92 90L109 72 96 55 110 39Z"/><path d="M76 27L85 37 75 51 85 63 76 77" fill="none"/>',
'blood_chalice':'<path d="M49 16H111V45Q107 64 84 67V83H102V91H58V83H76V67Q52 64 49 45Z"/><path d="M56 25H104V42Q81 58 56 42Z" fill="var(--dark)"/><path d="M30 24Q12 44 31 51 50 44 30 24Z"/>',
'moonbound_skin':'<path d="M108 8Q63 19 95 53 43 54 48 19 70 0 108 8Z"/><path d="M40 89L53 53 73 66 86 49 102 67 120 53 132 89Z"/><path d="M65 75L74 79M99 79L108 74M81 84L87 79 93 84" fill="none"/>',
'aether_amplifier':'<path d="M41 35H63V65H41ZM67 23H92V77H67ZM97 9H122V91H97Z"/><path d="M20 52H35M126 37Q148 53 128 69M127 24Q164 52 128 83" fill="none" stroke-width="4"/>',
'foundry_wisdom':'<path d="M27 31Q54 13 80 32 106 14 134 31V84Q107 68 80 88 54 70 27 84Z"/><path d="M80 34V86M38 42L68 44M39 57L67 59M96 43H121M96 57H119" fill="none"/><path d="M100 13L107 5 115 11 111 21Z"/>',
'dread_idol':'<path d="M57 20L81 8 105 20 112 57 97 67 109 91H51L64 67 48 57Z"/><path d="M59 35L74 42M89 42L104 34M65 53L81 46 97 53M67 79H94" fill="none"/><circle cx="81" cy="65" r="5" fill="var(--dark)"/>',
'primal_heart':'<path d="M80 92Q23 61 38 33 52 15 78 38 104 14 120 33 139 61 80 92Z"/><path d="M80 39L67 57 84 57 73 78 100 49 82 49 91 33" fill="var(--dark)"/><path d="M22 18L31 25M136 18L126 26M79 7V21" fill="none"/>',
}

def main():
    cards=tomllib.loads((ROOT/'content/cards/relic/gifts.toml').read_text(encoding='utf8'))['card']
    names=dict(line.split(' = ',1) for line in (ROOT/'content/i18n/pl.ftl').read_text(encoding='utf8').splitlines() if ' = ' in line)
    gallery=[]
    assert len(cards)==len(DRAWINGS)
    for card in cards:
        key=card['key'].removeprefix('card.');label=names[f'card-{key}-name'];school=next(iter(card['requires']));dark,light=PALETTE[school]
        svg=f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 160 100" role="img" aria-label="{escape(label)}"><title>{escape(label)}</title><defs><radialGradient id="bg"><stop stop-color="{dark}"/><stop offset="1" stop-color="#121222"/></radialGradient><linearGradient id="metal" x2="1" y2="1"><stop stop-color="#fff0c2"/><stop offset=".5" stop-color="{light}"/><stop offset="1" stop-color="{dark}"/></linearGradient></defs><rect width="160" height="100" fill="url(#bg)"/><ellipse cx="80" cy="50" rx="63" ry="43" fill="none" stroke="{light}" opacity=".25"/><g fill="url(#metal)" stroke="{light}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="--dark:{dark}">{DRAWINGS[key]}</g><path d="M5 18V5H20M140 5H155V18M5 82V95H20M140 95H155V82" fill="none" stroke="#e2c891"/></svg>'''
        (ROOT/f'art/cards/{key}.svg').write_text(svg,encoding='utf8',newline='\n')
        gallery.append(f'<article><img width="240" height="150" src="cards/{key}.svg" alt="{escape(label)}"><h2>{escape(label)}</h2></article>')
    (ROOT/'art/relic-review.html').write_text('<!doctype html><html lang="pl"><meta charset="utf-8"><title>Relic</title><style>body{background:#17121c;color:#ead5a3;font:16px Georgia,serif;padding:24px}main{display:grid;grid-template-columns:repeat(4,1fr);gap:20px}h2{font-size:17px}img{width:100%;height:auto}</style><h1>Relic — umagicznienia i dary</h1><main>'+''.join(gallery)+'</main></html>',encoding='utf8',newline='\n')

if __name__=='__main__': main()
