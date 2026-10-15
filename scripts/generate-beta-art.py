"""Original code-native Forge illustrations, authored as SVG paths, one composition per card."""
from html import escape
from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parent.parent
PALETTE = {"combat": ("#652b30", "#e6ae80"), "divine": ("#67512a", "#ffe3a0"), "element": ("#245458", "#a5e6cf"), "arcane": ("#36305d", "#cabffa"), "necro": ("#3e2c49", "#d8b7e3"), "tech": ("#22535c", "#a7e7df")}
# Common metal and accent materials unify the set. Geometry and composition are individual.
DRAWINGS = {
"shield_line": '<path d="M24 27L52 18 80 27V58Q66 77 52 84 37 75 24 58ZM65 29L94 20 122 29V61Q108 79 94 87 78 77 65 61Z"/><path d="M31 33L52 25 72 33V55L52 73 31 55M72 35L94 27 114 35V58L94 76 72 58" fill="var(--dark)"/><path d="M52 29V68M94 31V71"/>',
"war_drum": '<ellipse cx="80" cy="65" rx="38" ry="20"/><path d="M42 38V65Q80 92 118 65V38"/><ellipse cx="80" cy="38" rx="38" ry="18" fill="var(--dark)"/><path d="M43 42L62 72 80 54 99 73 118 42M60 25L98 61M105 20L67 58"/><circle cx="59" cy="24" r="5"/><circle cx="106" cy="19" r="5"/>',
"measured_strike": '<circle cx="112" cy="45" r="27" fill="none"/><circle cx="112" cy="45" r="16" fill="none"/><circle cx="112" cy="45" r="5"/><path d="M25 82L42 66 104 25 87 55 43 78Z"/><path d="M34 64L51 83M24 80L34 90M42 73L88 39"/>',
"shoulder_turn": '<path d="M25 68Q30 34 60 35L78 53 61 80ZM93 47L117 29 139 45 133 72 109 79Z"/><path d="M62 20Q98 3 126 24M119 13L129 26 114 27M30 84Q64 103 97 80M89 90L101 78 104 93" fill="none"/>',
"iron_sun": '<path d="M80 9L86 24 105 12 104 30 127 26 115 43 140 51 117 60 126 80 104 73 102 94 85 78 72 94 68 75 44 86 51 65 25 58 48 47 32 27 58 30 58 10 73 24Z"/><circle cx="81" cy="51" r="26" fill="var(--dark)"/><path d="M81 27L91 53 81 74 71 53Z"/>',
"last_standard": '<path d="M42 12V91M42 16Q73 5 92 17T134 18L120 34 134 50Q108 58 90 45T43 47"/><path d="M79 20L90 30 81 41 70 30Z" fill="var(--dark)"/><path d="M26 90H61M32 84L42 74 52 84"/>',
"oathbreaker": '<path d="M61 21A27 27 0 1 0 66 74M80 29A27 27 0 1 1 81 80" fill="none" stroke-width="8"/><path d="M64 17L74 27 62 35 75 43 66 52 77 64 64 81" fill="none"/><path d="M99 8L114 21 92 63 80 68 81 54Z"/>',
"razors_arc": '<path d="M25 75Q38 9 111 16L137 29Q67 24 51 82Z"/><path d="M42 69Q53 33 111 24" fill="none"/><path d="M43 76L38 94 23 90 30 72Z"/><path d="M66 63Q94 73 126 48" fill="none"/>',
"sanctuary_bell": '<path d="M48 68L55 56V39Q55 16 81 16T107 39V56L115 68Z"/><ellipse cx="81" cy="69" rx="34" ry="8" fill="var(--dark)"/><circle cx="81" cy="73" r="8"/><path d="M81 17V9M36 30L28 24M124 30L133 24M34 59H23M128 59H139" fill="none"/>',
"ember_liturgy": '<path d="M25 45Q49 32 80 48 109 31 136 44V86Q110 74 80 90 49 74 25 86Z"/><path d="M80 49V89M34 53L67 56M34 64L67 67M92 56L127 53M92 67L127 64" fill="none"/><path d="M77 42Q53 29 75 8 71 26 88 17 108 37 77 42Z"/>',
"pilgrim_scrip": '<path d="M43 35Q80 23 117 35L128 79Q80 97 32 79Z"/><path d="M44 39L80 57 116 39 106 24H54Z" fill="var(--dark)"/><path d="M61 27V16Q80 5 99 16V27" fill="none"/><rect x="73" y="46" width="15" height="19" rx="3"/><path d="M77 52H83V59H77Z" fill="var(--dark)"/>',
"broken_vow": '<path d="M61 23A28 28 0 1 0 62 78M88 23A28 28 0 1 1 88 78" fill="none" stroke-width="9"/><path d="M72 15L81 30 69 45 83 59 70 83M81 3V10M67 90L64 96" fill="none"/>',
"radiant_bastion": '<path d="M39 86V35H51V17H65V35H93V17H108V35H122V86Z"/><path d="M70 86V66Q80 50 92 66V86M54 44V57M108 44V57" fill="var(--dark)"/><path d="M15 85H145M80 5V22M19 22L33 31M141 22L128 31" fill="none"/>',
"absolving_flame": '<path d="M82 87Q39 86 47 48 52 61 61 60 53 36 82 7 78 32 101 42 115 28 112 22 144 82 82 87Z"/><path d="M83 79Q61 75 70 56L82 39Q77 61 96 66 101 79 83 79Z" fill="var(--dark)"/>',
"borrowed_dawn": '<circle cx="83" cy="35" r="21"/><path d="M31 72L56 61 80 64 94 57 114 59 131 48 138 53 116 77 86 87 56 80 36 88Z"/><path d="M68 68L89 72 106 66M83 5V10M48 18L57 24M110 13L105 21M116 36H127" fill="none"/>',
"suns_judgment": '<circle cx="80" cy="24" r="14"/><path d="M80 39V83M46 87H114M29 42H131M42 42L26 68H58ZM118 42L102 68H134Z"/><path d="M26 69Q42 86 58 69M102 69Q118 86 134 69" fill="var(--dark)"/>',
"rainwall": '<path d="M26 31Q16 15 36 14 44 2 62 12 81 5 88 24 112 12 123 30Z"/><path d="M34 39L27 54M52 39L44 60M72 37L64 52M108 40L99 60M122 41L113 54" fill="none"/><path d="M55 55L82 46 108 55V72Q91 90 82 93 66 85 55 72Z" fill="var(--dark)"/>',
"frost_needles": '<path d="M29 83L43 18 53 64 74 6 86 59 116 16 108 68 138 37 121 88Z"/><path d="M43 30L41 74M74 19L75 79M115 29L97 77M127 51L111 83" fill="none"/>',
"ashfall": '<path d="M79 88V32M79 55L51 37 37 16M77 59L108 36 120 16M52 38L27 39M105 39L133 42" fill="none" stroke-width="6"/><path d="M22 72L42 67 61 75 91 65 122 77 143 70V91H22Z"/><circle cx="46" cy="53" r="3"/><circle cx="117" cy="57" r="4"/><circle cx="92" cy="18" r="3"/>',
"lodestone": '<path d="M37 22H61V56Q80 77 99 56V22H124V60Q80 112 37 60Z"/><path d="M37 23V39H61V23M99 23V39H124V23" fill="var(--dark)"/><path d="M68 15L79 25 71 35M90 14L81 24 91 34" fill="none"/>',
"glacier_heart": '<path d="M80 82L36 42 48 17 68 15 80 28 92 14 115 20 126 43Z"/><path d="M80 28V82L59 38 48 18M80 82L102 39 114 21M38 42L59 38 80 28 102 39 125 43" fill="var(--dark)"/>',
"thunderhead": '<path d="M28 45Q11 25 33 21 37 5 60 15 79 1 96 19 115 10 124 26 147 32 134 45Z"/><path d="M77 39L57 67 78 65 67 93 105 55 82 57 94 39Z"/><path d="M37 55L31 68M121 55L113 72" fill="none"/>',
"volcanic_accord": '<path d="M23 88L65 36H95L140 88Z"/><path d="M65 36L80 51 95 36M79 51L70 70 83 65 91 83" fill="none"/><path d="M64 27Q47 10 64 8M80 25V5M97 27Q114 11 99 6" fill="none"/><path d="M30 92Q81 82 132 92" fill="none"/>',
"eye_of_storm": '<path d="M27 49Q74 6 132 47 85 90 27 49Z"/><circle cx="80" cy="48" r="20" fill="var(--dark)"/><path d="M91 34Q64 22 62 48T90 69Q111 56 97 44T77 45Q71 56 85 57" fill="none"/><path d="M17 23Q70 0 135 18M22 78Q84 104 143 73" fill="none"/>',
"mirror_ink": '<path d="M59 43L49 61V85Q80 95 111 85V61L101 43V25H59Z"/><path d="M57 70Q79 56 104 68V81Q80 89 57 81Z" fill="var(--dark)"/><rect x="57" y="19" width="46" height="11" rx="3"/><path d="M113 13L130 24 115 37 99 26Z"/><path d="M70 47L64 58"/>',
"splintered_second": '<path d="M75 14A37 37 0 1 0 74 88L63 69 77 57 65 45 80 33ZM90 14A37 37 0 0 1 91 86L80 67 95 56 81 44 96 31Z"/><path d="M49 50H65M55 27L66 37M51 73L63 62M113 34L102 42" fill="none"/>',
"apprentice_sigil": '<path d="M43 15H116V88H43Z"/><path d="M51 15V88M60 27H104M62 76H103" fill="none"/><path d="M82 34L102 48 95 69H69L62 48Z" fill="var(--dark)"/><path d="M82 39L94 61H70Z"/>',
"quiet_proof": '<path d="M44 23H117Q130 23 130 35V81H56Q42 81 42 68V30Q42 17 30 20T20 36H42M56 81Q62 81 61 67H42"/><path d="M61 36H113M61 46H104M76 59L83 68 105 52" fill="none"/>',
"ninefold_lens": '<circle cx="61" cy="48" r="27"/><circle cx="85" cy="44" r="27" fill="var(--dark)"/><circle cx="107" cy="40" r="27" fill="none"/><path d="M42 72L31 90M107 66L128 88M63 42L85 55 107 34" fill="none"/>',
"hourglass_sand": '<path d="M45 15H116M45 86H116M52 16Q51 39 77 50 51 61 52 86M109 16Q110 39 84 50 110 61 109 86" fill="none" stroke-width="6"/><path d="M61 33H101L81 48ZM80 55L61 79H101Z"/><path d="M80 17V26M80 50V60"/>',
"memory_vault": '<rect x="35" y="16" width="91" height="72" rx="7"/><rect x="44" y="25" width="72" height="54" rx="4" fill="var(--dark)"/><circle cx="84" cy="51" r="19"/><path d="M84 33V70M66 51H103M72 38L97 64M72 64L97 38M48 36H54M48 65H54" fill="none"/>',
"sealed_horizon": '<path d="M27 73L53 46 80 57 104 39 135 74Z"/><path d="M20 80H142M33 23H128" fill="none"/><circle cx="80" cy="26" r="15"/><rect x="69" y="22" width="23" height="25" rx="3" fill="var(--dark)"/><path d="M74 21V13Q80 4 87 13V21M80 29V38" fill="none"/>',
"blood_pledge": '<path d="M28 69L54 57 70 62 98 56 120 61 135 54 141 62 112 80 77 88 48 79 30 88Z"/><path d="M77 17Q54 44 79 49 104 43 77 17Z" fill="var(--dark)"/><path d="M60 68L82 73 102 65M77 6V12" fill="none"/>',
"lantern_of_ashes": '<path d="M59 23L49 37 56 81H106L113 37 102 23Z"/><path d="M66 23V13Q82 2 96 13V23M51 37H112M62 38V75M101 38V75M56 81L50 88H112L106 81" fill="none"/><path d="M82 70Q63 64 81 43 80 55 92 55 100 69 82 70Z" fill="var(--dark)"/>',
"carrion_wind": '<path d="M22 23Q60 7 84 27T141 23M17 76Q48 61 78 79T141 75" fill="none"/><path d="M55 46Q54 25 80 25T106 46V62L94 67V79H68V67L55 62Z"/><circle cx="69" cy="49" r="7" fill="var(--dark)"/><circle cx="92" cy="49" r="7" fill="var(--dark)"/><path d="M77 65L81 57 85 65M76 72V80M86 72V80" fill="none"/>',
"marrow_buckler": '<path d="M43 25L80 12 119 25V58Q106 78 81 89 54 79 43 58Z"/><path d="M62 30L103 68M103 30L62 68" fill="none" stroke-width="8"/><circle cx="59" cy="27" r="6"/><circle cx="105" cy="71" r="6"/><circle cx="105" cy="27" r="6"/><circle cx="59" cy="71" r="6"/>',
"silent_court": '<path d="M49 88V37L38 25 49 14 65 34H96L112 14 124 25 111 38V88H100V70H61V88Z"/><path d="M61 63V39H99V63Z" fill="var(--dark)"/><path d="M52 58H38V72H61M109 58H123V72H100M66 79H95" fill="none"/>',
"raven_feast": '<path d="M30 76Q45 62 45 42 21 37 14 22 44 22 67 45 76 16 94 17 108 16 113 29L137 36 112 41Q108 66 91 73L82 88 65 89 72 70 49 81Z"/><path d="M49 43L71 61 94 41M94 23L103 25M87 74L102 88M80 88H111" fill="none"/>',
"debt_of_dust": '<path d="M54 21H107L101 35Q124 57 109 76 82 96 54 76 39 57 62 35Z"/><path d="M59 29H104M66 40H96" fill="none"/><ellipse cx="36" cy="78" rx="16" ry="7"/><ellipse cx="123" cy="85" rx="19" ry="7"/><path d="M36 72V82M123 79V89"/>',
"hollow_crown": '<path d="M30 32L55 48 64 18 80 42 100 15 108 49 134 28 120 80H45Z"/><path d="M43 68Q80 57 123 67M49 79Q80 70 118 79" fill="none"/><path d="M81 48L89 58 81 68 73 58Z" fill="var(--dark)"/>',
"pressure_seal": '<circle cx="81" cy="51" r="35"/><circle cx="81" cy="51" r="23" fill="var(--dark)"/><path d="M44 45H24V58H44M118 45H138V58H118M73 15V5H88V15M74 87V96H88V87"/><path d="M64 51H98M81 34V68" fill="none"/>',
"spark_lattice": '<path d="M36 18L126 84M36 84L126 18M27 51H137M81 9V93" fill="none"/><circle cx="36" cy="18" r="7"/><circle cx="126" cy="84" r="7"/><circle cx="36" cy="84" r="7"/><circle cx="126" cy="18" r="7"/><path d="M85 27L66 53H83L76 76 102 46H85Z" fill="var(--dark)"/>',
"copper_memory": '<rect x="43" y="22" width="76" height="58" rx="5"/><rect x="60" y="34" width="43" height="33" fill="var(--dark)"/><path d="M51 8V22M69 8V22M90 8V22M110 8V22M51 80V94M69 80V94M90 80V94M110 80V94M27 33H43M27 51H43M27 68H43M119 33H136M119 51H136M119 68H136M67 44H96M67 57H96" fill="none"/>',
"lockstep_gear": '<path d="M47 21V10H64V21L77 27 87 21 97 35 88 44V60L97 69 86 83 75 77 63 83V93H47V82L34 76 24 83 14 69 23 60V44L14 35 24 21 34 27Z"/><circle cx="55" cy="51" r="20" fill="var(--dark)"/><circle cx="55" cy="51" r="7"/><circle cx="118" cy="51" r="25"/><circle cx="118" cy="51" r="13" fill="var(--dark)"/><path d="M118 21V13M118 81V89M143 51H152"/>',
"furnace_core": '<path d="M42 86V35L56 20H105L120 35V86Z"/><rect x="53" y="38" width="55" height="40" rx="5" fill="var(--dark)"/><path d="M81 70Q63 65 78 44 79 55 92 50 103 70 81 70Z"/><path d="M58 85V95M103 85V95M64 20V9H97V20M25 41H41M120 41H138" fill="none"/>',
"prism_cuirass": '<path d="M53 17L70 30H92L108 17 131 38 112 54 112 86H49V54L30 38Z"/><path d="M70 30L61 54 81 82 101 54 92 30M49 54H112M81 31V81" fill="var(--dark)"/><path d="M33 38L51 30M113 30L128 40" fill="none"/>',
"zero_point_coil": '<path d="M33 22H126M33 81H126M51 23Q113 27 112 39 109 52 50 52 15 52 52 61 113 68 112 77H51" fill="none" stroke-width="7"/><path d="M80 14V5M80 88V96M137 37L129 51 140 48 132 65M22 38L14 52 25 50 18 65" fill="none"/>',
"reset_lever": '<path d="M40 70H115L135 86H24Z"/><path d="M64 70L88 22 102 28 81 70Z"/><ellipse cx="96" cy="22" rx="23" ry="11"/><circle cx="74" cy="74" r="8" fill="var(--dark)"/><path d="M44 43Q34 29 51 16M44 16H55V28" fill="none"/>',
}

def main():
    cards=tomllib.loads((ROOT/'content/cards/forge/second_packet.toml').read_text(encoding='utf8'))['card']
    texts=dict(line.split(' = ',1) for line in (ROOT/'content/i18n/pl.ftl').read_text(encoding='utf8').splitlines() if ' = ' in line)
    assert len(DRAWINGS)==len(cards)==48
    previews=[]
    for card in cards:
        key=card['key'].removeprefix('card.');label=texts[f'card-{key}-name'];school=next(iter(card['requires']));dark,light=PALETTE[school]
        svg=f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 160 100" role="img" aria-label="{escape(label)}"><title>{escape(label)}</title><defs><radialGradient id="bg"><stop stop-color="{dark}"/><stop offset="1" stop-color="#171523"/></radialGradient><linearGradient id="metal" x2="1" y2="1"><stop stop-color="#f7e2b2"/><stop offset=".45" stop-color="{light}"/><stop offset="1" stop-color="{dark}"/></linearGradient></defs><rect width="160" height="100" fill="url(#bg)"/><circle cx="80" cy="50" r="42" fill="none" stroke="{light}" opacity=".25"/><g style="--dark:{dark}" fill="url(#metal)" stroke="{light}" stroke-width="2" stroke-linejoin="round" stroke-linecap="round">{DRAWINGS[key]}</g><path d="M5 17V5H18M142 5H155V17M5 83V95H18M142 95H155V83" fill="none" stroke="#e5cf9b" stroke-width="1.3"/></svg>'''
        (ROOT/f'art/cards/{key}.svg').write_text(svg,encoding='utf8',newline='\n')
        previews.append(f'<article><img width="240" height="150" src="cards/{key}.svg" alt="{escape(label)}"><h2>{escape(label)}</h2><p>{school} · {card["rarity"]}</p></article>')
    html='<!doctype html><html lang="pl"><meta charset="utf-8"><title>Forge — druga paczka</title><style>body{background:#17121c;color:#ebd6a2;font:16px Georgia,serif;padding:20px}main{display:grid;grid-template-columns:repeat(4,1fr);gap:24px}article{min-width:0}h2{font-size:17px}img{width:100%;height:auto}</style><h1>Forge — 48 nowych ilustracji</h1><main>'+''.join(previews)+'</main></html>'
    (ROOT/'art/forge-beta-review.html').write_text(html,encoding='utf8',newline='\n')

if __name__=='__main__':main()
