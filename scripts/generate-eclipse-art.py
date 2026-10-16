"""Original low-cost Eclipse objects, school counterseals and delayed-spell illustrations."""
from pathlib import Path
from html import escape
import importlib.util
import tomllib

ROOT=Path(__file__).resolve().parent.parent
spec=importlib.util.spec_from_file_location('portraits',Path(__file__).with_name('generate-full-portraits.py'))
portraits=importlib.util.module_from_spec(spec);spec.loader.exec_module(portraits)
PALETTE,GLYPHS=portraits.PALETTE,portraits.GLYPHS
DRAWINGS={
'marchers_canteen':'<path d="M60 13H101V28L117 43V77Q80 101 43 77V43L60 28Z"/><path d="M69 21H93M51 49Q81 34 110 49V73Q81 90 51 73Z" fill="var(--dark)"/><path d="M55 39L105 83M50 70L106 33" fill="none"/>',
'split_guard':'<path d="M44 23L74 13V88Q53 78 44 57ZM87 13L117 23V57Q107 79 87 88Z"/><path d="M79 18L72 34 87 47 73 61 86 79" fill="none"/><path d="M53 29L66 24V66M108 29L95 24V66" fill="none"/>',
'rusted_edge':'<path d="M44 81L98 20 114 9 105 32 65 77ZM36 71L64 91M36 83L29 93"/><path d="M62 67L86 38 83 48 94 41M77 50L69 48" fill="none" stroke="var(--dark)"/>',
'vigilant_step':'<path d="M56 18H91V49L121 65 137 79 133 89H52L39 77 51 50Z"/><path d="M52 51L83 59M51 64L80 73M49 79H121M68 25H83V43H68Z" fill="none"/><path d="M20 21Q30 9 41 22M121 34L139 22" fill="none"/>',
'votive_thread':'<path d="M61 26H103V75H61ZM51 18H112V29H51ZM51 74H112V85H51Z"/><path d="M63 33L101 40 63 48 101 56 63 64 101 72" fill="none" stroke="var(--dark)"/><path d="M63 62Q29 56 29 78T62 90M115 11L132 27M124 12L114 26" fill="none"/>',
'quiet_toll':'<path d="M46 68L54 54V37Q58 16 81 16 106 16 110 37V54L117 68Z"/><ellipse cx="81" cy="69" rx="36" ry="8" fill="var(--dark)"/><path d="M60 25L103 68M49 38L89 78" fill="none" stroke-width="7"/><path d="M81 16V7M25 36L37 43M124 42L137 35" fill="none"/>',
'shelter_pin':'<circle cx="79" cy="35" r="23"/><circle cx="79" cy="35" r="14" fill="var(--dark)"/><path d="M73 53L85 53 83 81 78 96 73 80ZM69 28L80 19 91 28V39Q82 49 80 49 72 47 69 39Z"/>',
'courage_token':'<circle cx="81" cy="50" r="36"/><circle cx="81" cy="50" r="27" fill="var(--dark)"/><path d="M81 26L98 49 88 69H74L64 49ZM80 31V64"/><path d="M27 25L36 34M124 67L134 77M25 75L37 66" fill="none"/>',
'moss_band':'<path d="M27 31Q80 8 135 31V74Q81 96 27 74Z"/><path d="M35 39Q81 24 127 39V65Q80 81 35 65Z" fill="var(--dark)"/><path d="M53 69Q33 48 50 40 65 43 60 62M72 72Q73 42 94 39 104 54 80 72M102 67Q103 49 120 48" fill="none"/>',
'cinder_fleck':'<path d="M50 81L37 58 61 46 63 18 87 31 114 18 108 46 129 65 106 82Z"/><path d="M61 72L68 47 79 58 89 40 103 66 90 77Z" fill="var(--dark)"/><circle cx="36" cy="26" r="3"/><circle cx="134" cy="42" r="4"/><circle cx="27" cy="75" r="2"/>',
'dew_pocket':'<path d="M38 50Q80 24 124 50L116 84Q81 100 45 84Z"/><path d="M44 52Q80 74 117 52M59 68L80 77 102 68" fill="none"/><path d="M80 7Q56 32 80 40 103 34 80 7Z"/><circle cx="75" cy="27" r="4" fill="var(--dark)"/>',
'silt_veil':'<path d="M31 21Q79 38 129 21L120 87Q80 69 40 87Z"/><path d="M40 53Q64 39 81 54T120 53M39 69Q64 55 81 70T120 69" fill="none"/><circle cx="57" cy="35" r="3"/><circle cx="101" cy="43" r="4"/>',
'ink_reservoir':'<path d="M58 14H103V30L117 48V82Q81 98 43 82V48L58 30Z"/><path d="M51 61Q80 45 110 61V78Q81 91 51 78Z" fill="var(--dark)"/><path d="M67 20H95M57 47L66 36" fill="none"/><path d="M124 13L142 24 126 41 108 29Z"/>',
'glass_rune':'<path d="M80 8L116 31 108 76 80 93 51 76 44 31Z"/><path d="M80 8L67 34 80 93 95 34ZM45 31L67 34 115 31" fill="var(--dark)"/><path d="M65 50L80 39 94 50 79 62 94 74" fill="none"/>',
'margin_note':'<path d="M42 11H104L124 32V90H42Z"/><path d="M103 12V34H122M51 29H88M52 47H107M52 61H93M52 75H102" fill="none"/><path d="M118 59L137 43 141 49 125 68 111 75Z" fill="var(--dark)"/>',
'pale_index':'<path d="M35 19H118L129 31V87H45L35 77Z"/><path d="M45 20V87M52 31H112M52 48H112M52 65H112" fill="none"/><path d="M107 20V63L97 53 88 63V20Z" fill="var(--dark)"/>',
'grave_ration':'<path d="M35 39L81 18 127 39V79L81 96 35 79Z"/><path d="M35 39L81 57 127 39M81 58V94M57 31L103 48" fill="none" stroke="var(--dark)"/><path d="M59 17L72 9M93 9L106 17M72 73H91" fill="none"/>',
'sable_splinter':'<path d="M43 88L67 45 78 9 88 42 115 63 89 58 72 82 62 95Z"/><path d="M76 27L76 55 55 85M85 49L101 57" fill="none" stroke="var(--dark)"/><circle cx="121" cy="23" r="5"/>',
'funeral_knot':'<path d="M80 49Q38 10 25 39 25 70 77 54 125 7 138 36 142 68 84 54 41 91 62 94 88 93 82 55 90 94 109 94 129 87 85 52Z" fill="none" stroke-width="7"/>',
'cold_obol':'<ellipse cx="81" cy="57" rx="40" ry="29"/><ellipse cx="81" cy="52" rx="39" ry="26"/><ellipse cx="81" cy="52" rx="29" ry="18" fill="var(--dark)"/><path d="M81 40V65M71 47L81 55 93 47M42 22L31 12M120 22L133 12M80 8V20" fill="none"/>',
'copper_switch':'<path d="M35 66H128V89H35ZM50 39H114V68H50Z"/><path d="M65 53L101 17 110 25 74 62Z" fill="var(--dark)"/><circle cx="108" cy="19" r="9"/><circle cx="49" cy="78" r="4" fill="var(--dark)"/><circle cx="114" cy="78" r="4" fill="var(--dark)"/>',
'spare_capacitor':'<path d="M63 17H99V79H63Z"/><ellipse cx="81" cy="17" rx="18" ry="7"/><path d="M68 23V73M92 23V73M72 79V96M90 79V94M38 37L50 49M36 63H50M122 35L110 47M112 63H129" fill="none"/>',
'washer_ward':'<path d="M80 9L111 19 128 50 112 80 80 92 48 80 32 50 48 19Z"/><circle cx="80" cy="50" r="23" fill="var(--dark)"/><circle cx="80" cy="50" r="12" fill="none"/><path d="M78 10V23M78 77V90M34 50H47M113 50H126" fill="none"/>',
'winding_key':'<circle cx="56" cy="40" r="24"/><circle cx="56" cy="40" r="13" fill="var(--dark)"/><path d="M76 47L129 77 123 88 112 81 108 87 98 81 102 75 73 59Z"/><path d="M40 22L48 30M51 16L55 27" fill="none"/>',
'eclipsed_verdict':'<circle cx="81" cy="26" r="20"/><path d="M89 9Q61 18 86 43 57 36 65 16Z" fill="var(--dark)"/><path d="M81 47V89M30 55H132M44 56L28 77H60ZM117 56L101 77H133ZM60 93H103"/>',
'grave_clock':'<circle cx="81" cy="50" r="39"/><circle cx="81" cy="50" r="30" fill="var(--dark)"/><path d="M68 48Q68 32 81 32T95 48V59H86V68H75V59H68Z"/><path d="M81 13V23M45 49H55M108 49H118M80 77V88" fill="none"/><path d="M73 48H78M85 48H90" stroke="var(--dark)"/>',
'zenith_pulse':'<circle cx="81" cy="50" r="16"/><circle cx="81" cy="50" r="29" fill="none"/><circle cx="81" cy="50" r="43" fill="none"/><path d="M20 50H43M118 50H142M81 3V16M81 84V98M43 14L52 24M112 77L124 89M42 88L52 78M112 23L123 12" fill="none"/>',
}
COUNTERS={'silence_of_iron','hushed_anthem','still_weather','blank_margin','wordless_dirge','null_spindle'}

def main():
    cards=tomllib.loads((ROOT/'content/cards/eclipse/first_packet.toml').read_text(encoding='utf8'))['card']
    names=dict(line.split(' = ',1) for line in (ROOT/'content/i18n/pl.ftl').read_text(encoding='utf8').splitlines() if ' = ' in line)
    assert {c['key'].removeprefix('card.')for c in cards}==DRAWINGS.keys()|COUNTERS
    gallery=[]
    for card in cards:
        key=card['key'].removeprefix('card.');label=names[f'card-{key}-name'];school=next(iter(card['requires']));dark,light=PALETTE[school]
        if key in COUNTERS:
            drawing=f'<circle cx="81" cy="50" r="35" fill="none" stroke-width="5"/><g transform="translate(81 50) scale(.8)" fill="{light}" style="--shade-detail:{dark}">{GLYPHS[school]}</g><path d="M32 13L54 29M110 72L134 90M33 90L54 72M110 28L134 12" fill="none" stroke-width="5"/>'
        else:drawing=DRAWINGS[key]
        svg=f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 160 100" role="img" aria-label="{escape(label)}"><title>{escape(label)}</title><defs><radialGradient id="bg" cx="50%" cy="40%"><stop stop-color="{dark}"/><stop offset="1" stop-color="#161321"/></radialGradient><linearGradient id="metal" x2="1" y2="1"><stop stop-color="#efe0bd"/><stop offset=".5" stop-color="{light}"/><stop offset="1" stop-color="{dark}"/></linearGradient></defs><rect width="160" height="100" fill="url(#bg)"/><circle cx="81" cy="50" r="43" fill="none" stroke="{light}" opacity=".2"/><path d="M10 82L54 14M108 86L151 18" fill="none" stroke="{light}" opacity=".1"/><g fill="url(#metal)" stroke="{light}" stroke-width="1.8" stroke-linejoin="round" stroke-linecap="round" style="--dark:{dark}">{drawing}</g><path d="M4 18V4H19M141 4H156V18M4 82V96H19M141 96H156V82" fill="none" stroke="#cfb580"/></svg>'''
        (ROOT/f'art/cards/{key}.svg').write_text(svg,encoding='utf8',newline='\n');gallery.append(f'<article><img width="240" height="150" src="cards/{key}.svg" alt="{escape(label)}"><h2>{escape(label)}</h2></article>')
    (ROOT/'art/eclipse-review.html').write_text('<!doctype html><html lang="pl"><meta charset="utf-8"><title>Eclipse</title><style>body{background:#17121c;color:#ead5a3;font:16px Georgia,serif;padding:24px}main{display:grid;grid-template-columns:repeat(4,1fr);gap:20px}h2{font-size:17px}img{width:100%;height:auto}</style><h1>Eclipse — pierwsza paczka</h1><main>'+''.join(gallery)+'</main></html>',encoding='utf8',newline='\n')

if __name__=='__main__':main()
