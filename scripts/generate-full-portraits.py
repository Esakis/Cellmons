"""Original remaining portraits and path variants, sharing the project's own base drawings."""
from pathlib import Path
from html import escape
import re
import json
import tomllib
import importlib.util
_spec=importlib.util.spec_from_file_location('beta',Path(__file__).with_name('generate-beta-portraits.py'))
_beta=importlib.util.module_from_spec(_spec);_spec.loader.exec_module(_beta)
BODY,FACE,PALETTE=_beta.BODY,_beta.FACE,_beta.PALETTE

ROOT=Path(__file__).resolve().parent.parent
PORTRAITS={
'tideform':('element','<path d="M38 179Q28 145 51 117 32 86 58 51 84 20 108 36 139 22 148 65 169 86 142 117 170 156 153 187Z" fill="#579b99" stroke="#c0e1c5" stroke-width="3"/><path d="M49 149Q88 123 144 148M60 173Q97 150 145 173" fill="none" stroke="#b3d7bf" stroke-width="4"/><path d="M65 81L85 86M111 85L131 80M83 107Q100 119 118 106" fill="none" stroke="#254c50" stroke-width="5"/><circle cx="110" cy="54" r="7" fill="#a7d4ba"/>'),
'silkblade':('combat',BODY+FACE+'<path d="M61 70L73 38 102 30 131 43 141 71 124 79 104 59 82 80Z" fill="#563e47" stroke="#c6b3a4" stroke-width="2"/><path d="M28 31L166 171M170 30L31 174M100 15V183M25 100H177" fill="none" stroke="#bfaebc" opacity=".5"/><path d="M39 199L61 134 74 119 78 138 56 200M147 199L123 139 125 119 140 134 163 200" fill="url(#metal)" stroke="#e4c79e" stroke-width="2"/>'),
'bone_sentinel':('necro',BODY+'<path d="M66 61Q70 30 102 31 138 34 139 66L132 106 115 113V132H87V111L70 104Z" fill="#cbbd9d" stroke="#62564e" stroke-width="3"/><path d="M79 73L96 77 93 91 78 89ZM109 77L128 72 128 88 112 91Z" fill="#3c3341"/><path d="M101 85L94 103H110ZM89 112V127M100 111V129M111 112V126" fill="none" stroke="#62564e" stroke-width="3"/><path d="M47 140L73 121 89 146 75 180 37 168M115 145L132 121 158 138 165 167 127 182" fill="url(#metal)" stroke="#c9b69b"/><path d="M47 197L53 159 98 165 145 158 153 197Z" fill="#5c5660" stroke="#d1c29e" stroke-width="3"/>'),
'veil_agent':('arcane',BODY+FACE+'<path d="M52 78Q68 16 114 28 142 37 151 85L122 72 93 60 73 86Z" fill="#473e5a" stroke="#ad9dc8" stroke-width="2"/><path d="M73 90L105 101 130 88 125 117 102 130 79 114Z" fill="#332c46"/><path d="M36 158L60 120 104 160 140 122 170 163 145 193 106 174 68 193Z" fill="#4a3d5b" stroke="#b2a0c1" stroke-width="2"/><path d="M170 200L153 87 148 65 143 88 156 200Z" fill="url(#metal)" stroke="#d4c5a1"/>'),
'aether_seer':('arcane',BODY+'<path d="M57 122Q48 45 83 22 121 9 146 65L149 125 128 139 74 138Z" fill="#b6adbf" stroke="#e0d2d9" stroke-width="2"/>'+FACE+'<path d="M66 68L82 29 103 16 129 33 139 67 114 51 87 56Z" fill="#595074"/><path d="M82 48L102 36 122 48 101 57Z" fill="#d6c9a0"/><path d="M62 126L96 176 83 200H124L112 172 144 126" fill="#746483" stroke="#c5afd2" stroke-width="2"/><circle cx="101" cy="155" r="13" fill="url(#metal)"/><circle cx="100" cy="153" r="6" fill="#bdb4e2"/><path d="M33 194V82M31 78L19 60 31 42 44 60Z" fill="url(#metal)" stroke="#ded0b1" stroke-width="3"/>'),
'jade_vanguard':('combat',BODY+FACE+'<path d="M62 66L74 31 99 26 132 38 140 67 120 73 82 73Z" fill="#426757" stroke="#d7c795" stroke-width="3"/><path d="M48 142L68 118 94 147 79 173 38 170ZM115 146L138 118 163 143 164 170 126 178Z" fill="#63816a" stroke="#ddcc9b" stroke-width="3"/><path d="M78 154L99 143 124 154 118 200H83Z" fill="#4d715b" stroke="#dbcb9c" stroke-width="3"/><path d="M100 152V200M84 169H118M85 184H116" fill="none" stroke="#c8bf8d" stroke-width="3"/><path d="M170 200V54M155 53L169 24 181 54Z" fill="url(#metal)" stroke="#e2ce9c" stroke-width="3"/>'),
}
GLYPHS={
'combat':'<path d="M-4 17V-10L0-23 4-10V17M-13 11H13M0 12V25"/>',
'divine':'<path d="M0-24V24M-23 0H23M-16-16L16 16M16-16L-16 16"/><circle r="13"/>',
'element':'<path d="M0-25Q-28 4-12 20 5 34 20 12 28-4 7-14 16 8-1 12-14 5 0-25Z"/>',
'arcane':'<path d="M0-25L7-8 25-7 11 6 16 24 0 14-17 24-11 6-25-7-7-8Z"/>',
'necro':'<path d="M-20-5Q-21-24 0-24T20-5V10L10 16V25H-10V16L-20 10Z"/><path d="M-13-7L-5-3M5-3L13-7M-5 12L0 4 5 12" fill="none" stroke="var(--shade-detail)"/>',
'tech':'<path d="M-10-24H10L12-13 23-10V10L12 13 10 24H-10L-12 13-23 10V-10L-12-13Z"/><circle r="10" fill="none" stroke="var(--shade-detail)"/>',
}

def frame(label,school,drawing):
    dark,light=PALETTE[school]
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 200" role="img" aria-label="{escape(label)}"><title>{escape(label)}</title><defs><radialGradient id="bg"><stop stop-color="{dark}"/><stop offset="1" stop-color="#1b1725"/></radialGradient><linearGradient id="metal" x2="1" y2="1"><stop stop-color="#e8d7ad"/><stop offset=".5" stop-color="{light}"/><stop offset="1" stop-color="#514a43"/></linearGradient></defs><rect width="200" height="200" rx="12" fill="url(#bg)"/><circle cx="100" cy="78" r="64" fill="none" stroke="{light}" opacity=".4"/><g style="--coat:{dark};--skin:#b89b80">{drawing}</g><rect x="4" y="4" width="192" height="192" rx="9" fill="none" stroke="#d2bb8c"/></svg>'''

def main():
    names=dict(line.split(' = ',1) for line in (ROOT/'content/i18n/pl.ftl').read_text(encoding='utf8').splitlines() if ' = ' in line)
    full=tomllib.loads((ROOT/'content/avatars/full.toml').read_text(encoding='utf8'))['avatar']
    paths=tomllib.loads((ROOT/'content/avatars/paths.toml').read_text(encoding='utf8'))['avatar']
    gallery=[];overlays={}
    for a in full:
        key=a['key'].removeprefix('avatar.');label=names[f'avatar-{key}-name']
        if a['rarity']=='shade':
            school=next(iter(a['schools']));dark,light=PALETTE[school]
            drawing=f'<path d="M23 199L37 138 64 110 60 61 79 26 106 16 135 44 144 97 134 114 169 146 181 199Z" fill="{dark}" stroke="{light}" stroke-width="3"/><path d="M65 66L82 42 108 32 127 57 134 88 119 109 83 107 69 88Z" fill="#161627"/><path d="M78 72L93 77M109 77L124 70" stroke="{light}" stroke-width="5"/><path d="M48 145L75 129 100 156 129 128 157 145 137 182 101 170 67 183Z" fill="{dark}" stroke="{light}" stroke-width="2"/><g transform="translate(101 155) scale(.6)" fill="{light}" stroke="{light}" stroke-width="2" style="--shade-detail:{dark}">{GLYPHS[school]}</g><path d="M54 179L41 198M143 177L160 198M101 176V199" stroke="{light}" opacity=".5" stroke-width="3"/>'
        else:school,drawing=PORTRAITS[key]
        (ROOT/f'art/avatars/{key}.svg').write_text(frame(label,school,drawing),encoding='utf8',newline='\n')
        gallery.append((key,label))
    for a in paths:
        key=a['key'].removeprefix('avatar.');base=re.sub(r'_path_[01]$','',key);label=names[f'avatar-{key}-name'];variant=a['path']['variant']
        school=next(iter(a['schools']));dark,light=PALETTE[school]
        svg=(ROOT/f'art/avatars/{base}.svg').read_text(encoding='utf8')
        svg=re.sub(r'aria-label="[^"]*"',f'aria-label="{escape(label)}"',svg,count=1)
        svg=re.sub(r'<title>.*?</title>',f'<title>{escape(label)}</title>',svg,count=1)
        badge='<circle cx="160" cy="160" r="30"/>' if variant==0 else '<path d="M132 136L160 125 188 136V165Q178 185 160 193 141 183 132 165Z"/>'
        overlay=f'<g fill="{dark}" stroke="{light}" stroke-width="3">{badge}<g transform="translate(160 159) scale(.7)" fill="{light}" stroke-width="2" style="--shade-detail:{dark}">{GLYPHS[school]}</g></g>'
        overlay+=f'<path d="M61 25L75 12 91 27 108 10 127 26 140 14" fill="none" stroke="{light}" stroke-width="{3 if variant else 1.5}"/><path d="M11 25V11H28M172 11H189V25M11 172V189H28" fill="none" stroke="{light}" stroke-width="3"/>'
        svg=svg.replace('</svg>',overlay+'</svg>')
        overlays[a['key']]=overlay
        (ROOT/f'art/avatars/{key}.svg').write_text(svg,encoding='utf8',newline='\n');gallery.append((key,label))
    html='<main>'+''.join(f'<article><img width="200" height="200" src="avatars/{key}.svg" alt="{escape(label)}"><h2>{escape(label)}</h2></article>' for key,label in gallery)+'</main>'
    (ROOT/'art/full-portraits-review.html').write_text('<!doctype html><html lang="pl"><meta charset="utf-8"><title>Pełne postacie i drogi</title><style>body{background:#17121c;color:#ead5a3;font:16px Georgia,serif;padding:24px}main{display:grid;grid-template-columns:repeat(5,1fr);gap:20px}h2{font-size:16px}img{width:100%;height:auto}</style><h1>Postacie, cienie i drogi</h1>'+html+'</html>',encoding='utf8',newline='\n')
    (ROOT/'art/path-overlays.json').write_text(json.dumps(overlays,ensure_ascii=False,indent=2)+'\n',encoding='utf8',newline='\n')

if __name__=='__main__':main()
