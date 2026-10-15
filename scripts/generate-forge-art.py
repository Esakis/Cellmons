"""Original SVG source drawings for the first Forge packet. Does not change earlier art."""
from html import escape
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PALETTES = {
    "combat": ("#7b211f", "#e8a581"), "divine": ("#604417", "#ffe6a6"),
    "element": ("#163e3d", "#91dbbb"), "arcane": ("#242653", "#bac5ff"),
    "necro": ("#34203e", "#d4a7e5"), "tech": ("#12454c", "#91e0df"),
}
DRAWINGS = {
    "tactical_reserve": ("combat", "Rezerwa taktyczna", """
      <path d="M15 76L30 40 45 76M115 76L130 38 146 76" fill="#31262a" stroke="#ba7553"/>
      <path d="M50 29Q80 15 110 29L115 81Q80 93 45 81Z" fill="#653c2c" stroke="#edc092" stroke-width="2"/>
      <path d="M48 42Q80 59 112 42L105 24H55Z" fill="#a77043" stroke="#edc092"/>
      <path d="M62 24Q62 8 80 8T98 24M58 43V80M102 43V80" fill="none" stroke="#d7b281" stroke-width="4"/>
      <rect x="74" y="43" width="12" height="14" rx="2" fill="url(#metal)"/><path d="M78 47H82V53H78Z" fill="#45311e"/>
      <path d="M73 66H87L90 79H70Z" fill="#86c9a3" stroke="#e1ffdb"/><path d="M76 62H84V68H76Z" fill="#d9c29d"/>
      <path d="M19 21V56M20 22L41 26 34 37 20 34" fill="#ad6e5b" stroke="#e9bb8a"/>
    """),
    "oath_of_steel": ("combat", "Przysięga stali", """
      <circle cx="80" cy="46" r="35" fill="none" stroke="#d8b567" stroke-width="2" stroke-dasharray="5 3"/>
      <path d="M28 90L45 26 52 12 57 28 38 92M132 90L115 26 108 12 103 28 122 92" fill="url(#metal)" stroke="#fff0bb"/>
      <path d="M51 30Q80 18 109 30L105 61Q97 78 80 88 63 78 55 61Z" fill="#34262a" stroke="url(#metal)" stroke-width="4"/>
      <path d="M80 28V80M56 46H105" stroke="#dfc482" stroke-width="2"/>
      <path d="M80 39L84 48 94 50 86 57 87 68 80 62 73 68 74 57 66 50 76 48Z" fill="url(#metal)"/>
      <path d="M42 82L57 79M118 82L103 79" stroke="#efc06b" stroke-width="5" stroke-linecap="round"/>
    """),
    "shared_vigil": ("divine", "Wspólne czuwanie", """
      <path d="M28 88V38Q80-8 132 38V88M40 88V42Q80 8 120 42V88" fill="none" stroke="#b29862" stroke-width="4"/>
      <path d="M10 88H150M15 94H145" stroke="#e1c687" stroke-width="3"/>
      <path d="M52 38V76M108 38V76" stroke="#f8d27e" stroke-width="3"/>
      <path d="M44 75H60L57 84H47ZM100 75H116L113 84H103Z" fill="url(#metal)"/>
      <ellipse cx="52" cy="35" rx="12" ry="20" fill="url(#glow)"/><ellipse cx="108" cy="35" rx="12" ry="20" fill="url(#glow)"/>
      <path d="M52 17Q40 34 52 40 64 34 52 17M108 17Q96 34 108 40 120 34 108 17" fill="#fff4bd"/>
      <path d="M64 57L80 46 96 57V76L80 85 64 76Z" fill="#735632" stroke="#ffe0a0"/><path d="M80 51V79M69 62H91" stroke="#ffe0a0" stroke-width="2"/>
    """),
    "dawn_reprieve": ("divine", "Wytchnienie świtu", """
      <circle cx="80" cy="38" r="31" fill="url(#glow)"/><circle cx="80" cy="38" r="17" fill="#fff3bc"/>
      <path d="M80 7V2M80 74V67M49 38H40M120 38H111M58 16L52 10M108 65L102 59M58 59L52 65M108 10L102 16" stroke="#f4d27c" stroke-width="2"/>
      <path d="M0 66Q40 50 80 64T160 62V100H0Z" fill="#3f4331"/>
      <path d="M11 78Q76 56 151 77M8 90Q78 65 152 91" fill="none" stroke="#b8a573"/>
      <path d="M56 74Q80 65 104 74L110 89H50Z" fill="url(#metal)" stroke="#fff0c0"/>
      <path d="M61 73Q80 84 99 73" fill="none" stroke="#4e432f" stroke-width="2"/>
      <path d="M29 91V55M29 62L22 55M29 69L36 60M130 91V55M130 62L123 54M130 69L137 61" stroke="#d9bc70" stroke-width="2"/>
    """),
    "cinder_volley": ("element", "Salwa żaru", """
      <path d="M0 88L35 67 56 86 88 65 121 81 160 62V100H0Z" fill="#19373c"/>
      <path d="M17 13Q58 22 113 57M16 37Q58 42 100 73M51 6Q91 13 133 38" fill="none" stroke="#ee7936" stroke-width="7"/>
      <path d="M17 13Q58 22 113 57M16 37Q58 42 100 73M51 6Q91 13 133 38" fill="none" stroke="#ffdb8e" stroke-width="2"/>
      <path d="M105 45Q130 48 126 67 102 72 103 55L113 58ZM94 62Q113 65 111 81 92 87 90 72L100 74ZM124 26Q146 29 144 47 125 51 120 38L134 40Z" fill="#ff9e42" stroke="#ffd69a"/>
      <circle cx="115" cy="59" r="5" fill="#fff3ce"/><circle cx="101" cy="74" r="4" fill="#fff3ce"/><circle cx="135" cy="39" r="4" fill="#fff3ce"/>
    """),
    "storm_braid": ("element", "Warkocz burzy", """
      <path d="M0 37Q10 14 33 29 50 1 73 24 96 2 118 23 146 8 160 34V51H0Z" fill="#2e444f" stroke="#71899a"/>
      <path d="M49 24Q106 47 59 67T79 95M109 24Q49 45 99 67T80 95" fill="none" stroke="#8de0c7" stroke-width="8"/>
      <path d="M49 24Q106 47 59 67T79 95M109 24Q49 45 99 67T80 95" fill="none" stroke="#edf9cf" stroke-width="2"/>
      <path d="M85 16L70 46H85L73 73 106 37H89L100 16Z" fill="#f5e49d"/>
      <path d="M27 39L18 62 29 58 20 80M135 39L143 62 132 58 143 80" fill="none" stroke="#a8dcff" stroke-width="2"/>
    """),
    "measured_thought": ("arcane", "Wyważona myśl", """
      <ellipse cx="80" cy="46" rx="50" ry="27" transform="rotate(-22 80 46)" fill="none" stroke="#d0b78b" stroke-width="2"/>
      <ellipse cx="80" cy="46" rx="24" ry="39" transform="rotate(25 80 46)" fill="none" stroke="#98b9ef" stroke-width="2"/>
      <circle cx="80" cy="46" r="14" fill="url(#glow)"/><circle cx="80" cy="46" r="7" fill="#e5edff"/>
      <circle cx="41" cy="31" r="5" fill="url(#metal)"/><circle cx="108" cy="23" r="4" fill="#b8d4f2"/><circle cx="123" cy="55" r="6" fill="#b695d1"/>
      <path d="M39 83L79 75 121 83 120 94 80 88 40 94Z" fill="#d8c6b5" stroke="#9e82ad"/><path d="M80 77V89M47 86L72 81M87 81L113 86" stroke="#614871"/>
      <path d="M80 18V10M80 82V74M51 46H44M117 46H109" stroke="#d3c6ff"/>
    """),
    "still_hour": ("arcane", "Zatrzymana godzina", """
      <circle cx="80" cy="49" r="39" fill="none" stroke="#aca4d7" stroke-dasharray="2 7"/>
      <path d="M55 12H105M55 88H105" stroke="url(#metal)" stroke-width="6" stroke-linecap="round"/>
      <path d="M60 16Q60 37 77 49 60 65 60 84H100Q100 65 83 49 100 37 100 16Z" fill="#5d5890" fill-opacity=".55" stroke="#d6dbff" stroke-width="2"/>
      <path d="M66 25H94L81 43H79Z" fill="#e7d38c"/><path d="M66 80L80 68 94 80Z" fill="#e7d38c"/>
      <circle cx="80" cy="50" r="2" fill="#fff1c1"/><circle cx="80" cy="59" r="2" fill="#fff1c1"/>
      <path d="M30 49H52M108 49H130M31 43L25 49 31 55M129 43L135 49 129 55" fill="none" stroke="#c0d6ff" stroke-width="2"/>
    """),
    "ash_tether": ("necro", "Więź popiołu", """
      <path d="M0 89Q29 70 59 90T120 86 160 91V100H0Z" fill="#251d2d"/>
      <path d="M20 84Q42 48 36 24M140 84Q118 48 124 24" fill="none" stroke="#6a5667" stroke-width="12"/>
      <path d="M26 31L23 12M34 26L33 7M42 27L45 10M118 27L115 10M126 26L127 7M134 31L137 12" stroke="#d6bfcb" stroke-width="4" stroke-linecap="round"/>
      <path d="M34 35Q60 57 80 48T126 35" fill="none" stroke="#a06fb3" stroke-width="9"/>
      <path d="M34 35Q60 57 80 48T126 35" fill="none" stroke="#e2b2ed" stroke-width="5" stroke-dasharray="6 5"/>
      <ellipse cx="80" cy="49" rx="16" ry="21" fill="url(#glow)"/>
      <path d="M73 70L80 60 87 70 80 81Z" fill="#e3c3e8"/>
    """),
    "grave_bloom": ("necro", "Grobowy rozkwit", """
      <path d="M14 90V52Q25 39 36 52V90M126 90V43Q137 31 148 43V90" fill="#3e3545" stroke="#75637e"/>
      <path d="M80 91Q72 68 80 49" fill="none" stroke="#c5c0a7" stroke-width="4"/>
      <path d="M76 73Q49 65 53 53 73 51 78 70M81 82Q106 67 110 79 103 92 82 86" fill="#576451" stroke="#c9c5aa"/>
      <ellipse cx="80" cy="38" rx="10" ry="22" fill="#b98fc9"/><ellipse cx="80" cy="38" rx="10" ry="22" transform="rotate(60 80 38)" fill="#d6a4df"/>
      <ellipse cx="80" cy="38" rx="10" ry="22" transform="rotate(120 80 38)" fill="#bc83c9"/>
      <circle cx="80" cy="38" r="10" fill="#e3dac4" stroke="#f7ecd8"/><path d="M75 35H77M83 35H85M77 42H83" stroke="#543e5e" stroke-width="2"/>
      <path d="M54 92Q80 84 108 94" stroke="#a697a8" fill="none"/>
    """),
    "brass_casing": ("tech", "Mosiężna osłona", """
      <path d="M28 30L42 23 47 72 35 80 22 62ZM132 30L118 23 113 72 125 80 138 62Z" fill="#94713b" stroke="#dfc78d"/>
      <path d="M49 17H111L117 63Q110 83 80 92 50 83 43 63Z" fill="url(#metal)" stroke="#ffe4a2" stroke-width="3"/>
      <path d="M57 25H103L108 60Q101 75 80 83 59 75 52 60Z" fill="#235b63" stroke="#83c9ca" stroke-width="2"/>
      <circle cx="80" cy="51" r="17" fill="#8d713f" stroke="#e5cb86" stroke-width="4"/>
      <circle cx="80" cy="51" r="8" fill="#88d7d9" stroke="#d5f6e6"/>
      <path d="M80 30V34M80 68V72M59 51H63M97 51H101" stroke="#e6d0a0" stroke-width="3"/>
      <circle cx="55" cy="21" r="2" fill="#4e472f"/><circle cx="105" cy="21" r="2" fill="#4e472f"/>
    """),
    "overdrive_valve": ("tech", "Zawór przeciążenia", """
      <path d="M8 67H45L53 52M152 67H115L107 52M23 94V76H55M137 94V76H105" fill="none" stroke="#927f4c" stroke-width="12"/>
      <path d="M8 67H45L53 52M152 67H115L107 52" fill="none" stroke="#f0d496" stroke-width="3"/>
      <circle cx="80" cy="52" r="28" fill="url(#metal)" stroke="#ffe5af" stroke-width="2"/>
      <circle cx="80" cy="52" r="21" fill="#122f39" stroke="#87bfc0"/>
      <path d="M65 48A16 16 0 0 1 94 44" fill="none" stroke="#9fdbcf" stroke-width="3"/>
      <path d="M87 38A16 16 0 0 1 96 52" fill="none" stroke="#e18b65" stroke-width="3"/>
      <path d="M80 52L91 39" stroke="#ffe3a3" stroke-width="3" stroke-linecap="round"/><circle cx="80" cy="52" r="4" fill="#ddbf7c"/>
      <path d="M54 28Q43 18 53 9M67 24Q60 13 70 5M104 27Q116 14 106 6" fill="none" stroke="#c0e5df" stroke-width="3" opacity=".7"/>
      <path d="M71 86H89V94H71Z" fill="#b5a16f"/>
    """),
}

def main():
    for key, (school, label, drawing) in DRAWINGS.items():
        dark, light = PALETTES[school]
        svg = f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 160 100" role="img" aria-label="{escape(label)}"><title>{escape(label)}</title>
<defs><radialGradient id="bg" cx="50%" cy="35%" r="85%"><stop stop-color="{light}"/><stop offset=".55" stop-color="{dark}"/><stop offset="1" stop-color="#111323"/></radialGradient><linearGradient id="metal" x2="1" y2="1"><stop stop-color="#fff1bd"/><stop offset=".4" stop-color="#bca073"/><stop offset="1" stop-color="#5b4934"/></linearGradient><radialGradient id="glow"><stop stop-color="#fff9d8" stop-opacity=".8"/><stop offset="1" stop-color="{light}" stop-opacity="0"/></radialGradient></defs>
<rect width="160" height="100" fill="url(#bg)"/><circle cx="80" cy="46" r="42" fill="url(#glow)"/>
<path d="M5 92L80 56 155 92M11 8L80 56 149 8" fill="none" stroke="{light}" opacity=".15"/>
{drawing}
<g fill="{light}" opacity=".7"><circle cx="18" cy="19" r="1"/><circle cx="141" cy="15" r="1.5"/><circle cx="23" cy="47" r=".8"/><circle cx="145" cy="84" r="1"/></g>
<rect x="3" y="3" width="154" height="94" rx="3" fill="none" stroke="#ead1a0" opacity=".65"/><path d="M5 15V5H15M145 5H155V15M5 85V95H15M145 95H155V85" fill="none" stroke="#f7e2b5" stroke-width="1.5"/></svg>'''
        (ROOT / "art/cards" / f"{key}.svg").write_text(svg, encoding="utf-8", newline="\n")

if __name__ == "__main__":
    main()
