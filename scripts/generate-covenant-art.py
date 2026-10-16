"""Original Covenant capstones and paired-school compositions; no external image material."""
from pathlib import Path
from html import escape
import importlib.util
import tomllib

ROOT=Path(__file__).resolve().parent.parent
spec=importlib.util.spec_from_file_location('portraits',Path(__file__).with_name('generate-full-portraits.py'))
portraits=importlib.util.module_from_spec(spec);spec.loader.exec_module(portraits)
PALETTE,GLYPHS=portraits.PALETTE,portraits.GLYPHS
DRAWINGS={
'final_brand':'<path d="M76 66V29L82 5 90 29V66M55 68H111M78 69V92H89V69"/><path d="M51 65Q26 53 46 24 45 44 59 35 65 22 60 15 87 38 67 58M109 63Q132 50 114 24 119 45 102 43" fill="none"/>',
'bastion_oath':'<path d="M26 85V39H39V22H54V39H107V22H123V39H136V85Z"/><path d="M58 48L81 40 105 48V68Q95 82 81 89 66 81 58 68Z" fill="var(--dark)"/><path d="M81 49V78M68 63H94" fill="none"/>',
'red_horizon':'<circle cx="81" cy="47" r="30"/><path d="M16 77Q76 57 143 78M20 88H139" fill="none"/><path d="M50 88L97 21 106 14 106 28 58 94Z" fill="var(--dark)"/><path d="M43 75L66 93" fill="none"/>',
'dawn_covenant':'<path d="M26 76L43 58 63 62 81 55 100 62 118 57 137 75 122 88 100 82 81 91 61 82 40 89Z"/><circle cx="81" cy="26" r="16" fill="none"/><path d="M81 3V8M48 11L56 17M115 11L107 17M34 33H47M114 33H129M61 68L81 76 101 68" fill="none"/>',
'sanctum_mirror':'<ellipse cx="82" cy="47" rx="33" ry="40"/><ellipse cx="82" cy="46" rx="25" ry="32" fill="var(--dark)"/><path d="M82 22V70M69 46H95M43 90H121M66 84L60 91M97 84L104 91" fill="none"/><path d="M45 15L30 6M120 15L134 6" fill="none"/>',
'light_balance':'<path d="M80 25V85M58 90H103M28 39H131M43 40L24 68H62ZM116 40L97 68H135Z"/><circle cx="80" cy="18" r="11"/><path d="M24 69Q43 85 62 69M97 69Q117 85 135 69" fill="var(--dark)"/>',
'crucible_sky':'<path d="M27 31Q19 11 39 13 51 1 68 13 91 0 105 15 128 7 138 30Z"/><path d="M54 54H108L102 82 80 92 60 82ZM80 30L66 51H83L74 69 104 41H85L95 30Z"/><path d="M23 45L37 58M140 45L128 58" fill="none"/>',
'rime_crown':'<path d="M33 73L25 35 49 50 62 18 81 45 101 18 113 50 139 34 129 73 112 88H50Z"/><path d="M39 70H124M52 86L63 72 81 85 100 72 113 86M62 19L66 56M101 19L96 55" fill="none" stroke="var(--dark)"/>',
'storm_confluence':'<path d="M26 28Q60 7 90 22T139 24M18 53Q56 31 89 48T142 48M25 81Q63 61 92 78T136 77" fill="none" stroke-width="7"/><path d="M83 17L61 52H80L69 85 107 42H85L98 17Z"/>',
'seventh_hour':'<circle cx="81" cy="49" r="38"/><circle cx="81" cy="49" r="30" fill="var(--dark)"/><path d="M81 23V50L67 67M80 12V18M44 49H50M112 49H118M81 80V86M54 22L59 28M106 73L111 78" fill="none"/><circle cx="81" cy="49" r="4"/>',
'astral_prism':'<path d="M80 8L115 31 105 73 80 92 54 73 45 31Z"/><path d="M80 8L67 40 80 92 94 40ZM46 31L67 40 114 31M54 73L67 40 105 73" fill="var(--dark)"/><path d="M23 14L37 26M125 26L139 14M18 61L39 59M123 59L144 61" fill="none"/>',
'folded_horizon':'<path d="M20 75L48 29 77 60 104 20 142 65 116 90 81 76 48 93Z"/><path d="M48 29V93M77 60L81 76M104 20L116 90M23 76L48 64 80 73 105 56 139 65" fill="none" stroke="var(--dark)"/>',
'mourning_throne':'<path d="M43 82V17L59 8 81 21 102 8 119 17V82ZM30 59H52V92H30ZM110 59H132V92H110Z"/><path d="M57 29H105V66H57Z" fill="var(--dark)"/><path d="M70 41L80 48 91 41M71 56H91M51 82H111" fill="none"/>',
'last_ember':'<path d="M49 24L59 11H101L112 24 105 72 91 79 83 94 72 80 55 73Z"/><path d="M51 27H109M63 28V65M98 28V64M59 77L78 69 91 79 108 72" fill="none"/><path d="M79 65Q62 53 78 36 77 47 88 44 99 59 79 65Z" fill="var(--dark)"/>',
'night_tithe':'<ellipse cx="48" cy="77" rx="26" ry="13"/><ellipse cx="80" cy="60" rx="26" ry="13"/><ellipse cx="111" cy="82" rx="26" ry="13"/><path d="M80 8Q59 35 80 41 103 36 80 8Z"/><path d="M37 76H57M69 59H90M101 81H122" fill="none" stroke="var(--dark)"/>',
'aether_reactor':'<path d="M49 18H112V84H49Z"/><path d="M39 27H49M39 43H49M39 59H49M39 75H49M112 27H123M112 43H123M112 59H123M112 75H123" fill="none" stroke-width="5"/><path d="M61 25H101V76H61Z" fill="var(--dark)"/><path d="M69 29L93 41 69 54 93 68M68 77L95 63 70 48 94 34" fill="none"/>',
'clockwork_aegis':'<path d="M45 24L81 11 119 24V59Q105 82 81 92 57 82 45 59Z"/><path d="M73 31H89L92 40 101 44V60L91 63 87 73H73L69 64 60 60V44L69 40Z" fill="var(--dark)"/><circle cx="81" cy="52" r="11"/><path d="M81 43V61M72 52H90" fill="none" stroke="var(--dark)"/>',
'convergence_engine':'<path d="M21 33L42 20 63 33V68L42 81 21 68ZM99 33L120 20 141 33V68L120 81 99 68Z"/><path d="M62 38H101V62H62Z"/><circle cx="81" cy="50" r="12" fill="var(--dark)"/><path d="M34 31L51 68M26 42L60 55M112 31L129 68M104 42L138 55" fill="none" stroke="var(--dark)"/>',
}
LINKS={
'sanctuary_steel':'<path d="M30 89H130M43 83L80 66 118 83M80 69V92"/>',
'monsoon_feint':'<path d="M19 81Q56 62 86 81T142 78M107 69L121 80 106 90"/>',
'steel_refrain':'<path d="M24 84Q60 62 98 83T141 80M21 92Q60 70 98 91T140 87"/>',
'marrow_oath':'<path d="M35 91L123 75M39 75L119 92"/><circle cx="35" cy="91" r="4"/><circle cx="122" cy="75" r="4"/>',
'gyroblade':'<ellipse cx="81" cy="50" rx="59" ry="37"/><path d="M80 72V94M71 88H90"/>',
'dew_censer':'<path d="M58 76H104L97 92H65ZM66 71Q57 62 66 55M81 71Q72 62 81 55M97 71Q88 62 97 55"/>',
'prayer_lens':'<path d="M27 85L72 70 89 84 134 68M76 77V95"/><circle cx="81" cy="78" r="10"/>',
'twilight_accord':'<path d="M26 77Q55 66 81 79 108 65 135 77V92Q108 82 81 95 54 82 26 92ZM81 79V95"/>',
'dawn_automaton':'<path d="M27 92V80L50 69 81 84 112 69 138 81V92M68 87H95"/>',
'resonant_cloud':'<path d="M22 83Q15 71 33 71 38 61 54 68 73 56 86 69 104 59 117 72 139 67 143 83Z"/>',
'ashroot_pact':'<path d="M80 69V91M80 81L54 73 29 85M81 85L110 74 137 88M55 74L48 91M109 74L118 93"/>',
'thought_gear':'<path d="M25 81L35 73 44 83 54 73 63 83 73 73 82 83 92 73 101 83 111 73 121 83 132 73 142 81M31 93H137"/>',
'thunder_socket':'<path d="M18 89H56L72 70 83 85 100 66 113 90H143M80 69V60"/>',
'umbral_parallax':'<path d="M26 92L68 69 119 91 136 70 85 94 43 73Z"/>',
'mortal_coil':'<path d="M25 83Q36 67 47 83T69 83 91 83 113 83 135 83M24 94H138"/>',
}

def main():
    cards=tomllib.loads((ROOT/'content/cards/covenant/first_packet.toml').read_text(encoding='utf8'))['card']
    names=dict(line.split(' = ',1) for line in (ROOT/'content/i18n/pl.ftl').read_text(encoding='utf8').splitlines() if ' = ' in line)
    assert {c['key'].removeprefix('card.') for c in cards}==DRAWINGS.keys()|LINKS.keys()
    gallery=[]
    for card in cards:
        key=card['key'].removeprefix('card.');label=names[f'card-{key}-name'];schools=list(card['requires']);dark,light=PALETTE[schools[0]];dark2,light2=PALETTE[schools[-1]]
        if key in DRAWINGS:drawing=DRAWINGS[key]
        else:
            drawing=f'<path d="M57 45Q80 25 102 45M58 55Q80 74 102 55" fill="none" opacity=".6"/>'
            for school,x in zip(schools,[47,113]):
                d,l=PALETTE[school]
                drawing+=f'<circle cx="{x}" cy="39" r="27" fill="{d}"/><g transform="translate({x} 39) scale(.7)" fill="{l}" stroke="{l}" style="--shade-detail:{d}">{GLYPHS[school]}</g>'
            drawing+=f'<g fill="none" stroke="{light2}" stroke-width="2.5">{LINKS[key]}</g>'
        svg=f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 160 100" role="img" aria-label="{escape(label)}"><title>{escape(label)}</title><defs><linearGradient id="bg" x2="1" y2="1"><stop stop-color="{dark}"/><stop offset="1" stop-color="{dark2}"/></linearGradient><linearGradient id="metal" x2="1" y2="1"><stop stop-color="#fff1c8"/><stop offset=".5" stop-color="{light}"/><stop offset="1" stop-color="{dark}"/></linearGradient></defs><rect width="160" height="100" fill="#171222"/><rect x="8" y="8" width="144" height="84" rx="24" fill="url(#bg)"/><circle cx="81" cy="50" r="42" fill="none" stroke="{light}" opacity=".2"/><g fill="url(#metal)" stroke="{light}" stroke-width="1.8" stroke-linejoin="round" stroke-linecap="round" style="--dark:{dark}">{drawing}</g><path d="M4 19V4H20M140 4H156V19M4 81V96H20M140 96H156V81" fill="none" stroke="#dbc38d"/></svg>'''
        (ROOT/f'art/cards/{key}.svg').write_text(svg,encoding='utf8',newline='\n');gallery.append(f'<article><img width="240" height="150" src="cards/{key}.svg" alt="{escape(label)}"><h2>{escape(label)}</h2><p>{" / ".join(schools)} · {card["rarity"]}</p></article>')
    (ROOT/'art/covenant-review.html').write_text('<!doctype html><html lang="pl"><meta charset="utf-8"><title>Covenant</title><style>body{background:#17121c;color:#ead5a3;font:16px Georgia,serif;padding:24px}main{display:grid;grid-template-columns:repeat(4,1fr);gap:20px}h2{font-size:17px}img{width:100%;height:auto}</style><h1>Covenant — 33 nowe karty</h1><main>'+''.join(gallery)+'</main></html>',encoding='utf8',newline='\n')

if __name__=='__main__':main()
