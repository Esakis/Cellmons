"""Three original class portraits; no source images or existing portrait files are modified."""
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PORTRAITS = {
    "cinder_knight": ("Rycerz żużlu", "#a74e39", "#281b26", """
<path d="M0 130L29 100 43 126 72 94 101 129 141 92 171 122 200 101V200H0Z" fill="#34252c"/>
<path d="M25 115L31 80 40 115M155 112L166 72 174 112" stroke="#8f6a56" fill="#281c22"/>
<path d="M36 194Q32 136 61 119L82 112H119L144 126Q172 145 174 200Z" fill="#823c31" stroke="#e5bd85" stroke-width="2"/>
<path d="M61 119L92 151 76 192 42 171ZM135 119L108 151 124 194 159 172Z" fill="url(#metal)" stroke="#d7bf94"/>
<path d="M72 66Q74 38 101 34 130 38 133 65L124 107 104 124 79 103Z" fill="#473136" stroke="#d9b98a" stroke-width="2"/>
<path d="M71 67L81 43 99 34 120 43 133 66 105 76Z" fill="url(#metal)"/>
<path d="M100 36L104 75V112" stroke="#ffe2a6" stroke-width="3"/>
<path d="M76 76L98 82 95 89 79 85M109 82L129 76 126 85 111 89" fill="#ffb169"/>
<path d="M80 93L96 96M108 96L123 91M83 101L96 104M109 103L119 99" stroke="#ad805b" stroke-width="2"/>
<path d="M93 153L103 142 113 153 109 169 97 169Z" fill="#d1a45b"/><circle cx="103" cy="157" r="4" fill="#ffce7a"/>
<path d="M24 181L39 127 47 115 50 130 35 186Z" fill="url(#metal)" stroke="#ffe2a6"/>
<path d="M26 178L41 184" stroke="#ce9f5d" stroke-width="5"/>
"""),
    "copper_distiller": ("Destylator", "#318089", "#18262e", """
<path d="M19 157V35H43V75M162 149V24H185V112" fill="none" stroke="#a78b58" stroke-width="12"/>
<path d="M19 157V35H43M162 149V24H185" fill="none" stroke="#e4c990" stroke-width="3"/>
<circle cx="43" cy="80" r="17" fill="#3b6b6b" stroke="#d3b274" stroke-width="4"/>
<path d="M30 200Q33 145 68 126L87 116H122L143 131Q168 155 173 200" fill="#40575a" stroke="#a8bcb0" stroke-width="2"/>
<path d="M81 119L100 150 124 119 134 200H68Z" fill="#a28053"/>
<path d="M71 53Q100 27 128 57L124 102Q116 119 100 125 84 118 76 101Z" fill="#b98d70" stroke="#513d38" stroke-width="2"/>
<path d="M67 65L74 40Q101 29 128 48L134 70Z" fill="#4f4c41" stroke="#ccb888" stroke-width="2"/>
<path d="M70 72H133" stroke="#6e5039" stroke-width="10"/>
<circle cx="84" cy="73" r="13" fill="#618f8b" stroke="url(#metal)" stroke-width="5"/><circle cx="119" cy="73" r="13" fill="#618f8b" stroke="url(#metal)" stroke-width="5"/>
<path d="M78 68L88 77M113 68L123 77" stroke="#c8f1e2" stroke-width="2"/>
<path d="M98 82L94 95 105 96M91 108Q101 112 111 106" fill="none" stroke="#694d43" stroke-width="2"/>
<path d="M42 170Q58 157 79 166L84 184 64 190 45 184" fill="#ae856b" stroke="#513d38"/>
<path d="M135 149V164L147 180Q150 191 139 194H118Q109 188 117 177L126 164V149Z" fill="#70caba" fill-opacity=".85" stroke="#d0eee1" stroke-width="2"/>
<path d="M125 150H136M120 181H145" stroke="#e6d3a1" stroke-width="4"/>
"""),
    "ruin_keeper": ("Strażnik ruin", "#617c5a", "#202b2b", """
<path d="M20 176V62Q22 24 57 19M180 176V62Q178 24 143 19" fill="none" stroke="#718077" stroke-width="20"/>
<path d="M18 83L33 69M169 51L181 64M19 126L31 112M169 138L182 151" stroke="#b9bca0" stroke-width="2"/>
<path d="M17 182Q28 146 34 37M180 188Q164 145 174 40" fill="none" stroke="#81955f" stroke-width="4"/>
<path d="M32 66Q52 47 47 72L32 82M168 96Q148 78 150 105L168 113" fill="#91a868"/>
<path d="M31 200Q34 142 65 118L86 111H121L145 129Q170 154 172 200Z" fill="#56664e" stroke="#b3b995" stroke-width="2"/>
<path d="M64 120Q55 51 85 34 117 22 136 64L137 125 118 148 76 144Z" fill="#465441" stroke="#9ca581" stroke-width="2"/>
<path d="M77 66Q103 43 126 68L121 103 103 119 84 103Z" fill="#b4aa8e" stroke="#4a4b40"/>
<path d="M78 79L96 84M108 84L123 79" stroke="#5c6556" stroke-width="3"/>
<path d="M101 83L97 96 106 97M90 104L103 129 116 103" fill="#d6d1b6"/>
<path d="M65 126L88 163 108 151 135 125M54 181L88 163 119 185" fill="none" stroke="#aaa880" stroke-width="3"/>
<circle cx="105" cy="153" r="10" fill="url(#metal)"/><path d="M105 145V161M100 149L110 157" stroke="#5b6550" stroke-width="2"/>
<path d="M40 200L43 73M43 75L29 57M43 74L60 56" fill="none" stroke="#b29a6d" stroke-width="7"/>
<path d="M29 57Q40 32 59 56 52 76 43 75Z" fill="#797697" stroke="#cec2d8" stroke-width="2"/>
"""),
}

def main():
    for key, (label, light, dark, drawing) in PORTRAITS.items():
        svg=f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 200" role="img" aria-label="{label}"><title>{label}</title><defs><radialGradient id="bg" cx="50%" cy="35%" r="80%"><stop stop-color="{light}"/><stop offset="1" stop-color="{dark}"/></radialGradient><linearGradient id="metal" x2="1" y2="1"><stop stop-color="#efe3bd"/><stop offset=".5" stop-color="#ac9568"/><stop offset="1" stop-color="#4f493f"/></linearGradient></defs><rect width="200" height="200" rx="12" fill="url(#bg)"/><circle cx="102" cy="70" r="58" fill="none" stroke="#ddcf9f" stroke-width="1" opacity=".35"/>{drawing}<rect x="4" y="4" width="192" height="192" rx="9" fill="none" stroke="#d5bd88" opacity=".7"/><path d="M12 30V12H30M170 12H188V30M12 170V188H30M170 188H188V170" stroke="#e1c894" fill="none"/></svg>'''
        (ROOT/'art/avatars'/f'{key}.svg').write_text(svg,encoding='utf-8',newline='\n')

if __name__ == '__main__':
    main()
